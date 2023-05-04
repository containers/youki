use nix::unistd::{self, close};
use std::env;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub const NOTIFY_FILE: &str = "notify.sock";

#[derive(Debug, thiserror::Error)]
pub enum NotifyListenerError {
    #[error("failed to chdir when create notify socket")]
    Chdir { source: nix::Error, path: PathBuf },
    #[error("invalid path: {0}")]
    InvalidPath(PathBuf),
    #[error("failed to bind notify socket: {name}")]
    Bind {
        source: std::io::Error,
        name: String,
    },
    #[error("failed to connect to notify socket: {name}")]
    Connect {
        source: std::io::Error,
        name: String,
    },
    #[error("failed to get cwd")]
    GetCwd(#[source] std::io::Error),
    #[error("failed to accept notify listener")]
    Accept(#[source] std::io::Error),
    #[error("failed to close notify listener")]
    Close(#[source] nix::errno::Errno),
    #[error("failed to read notify listener")]
    Read(#[source] std::io::Error),
    #[error("failed to send start container")]
    SendStartContainer(#[source] std::io::Error),
}

type Result<T> = std::result::Result<T, NotifyListenerError>;

pub struct NotifyListener {
    socket: UnixListener,
}

impl NotifyListener {
    pub fn new(socket_path: &Path) -> Result<Self> {
        // Unix domain socket has a maximum length of 108, different from
        // normal path length of 255. Due to how docker create the path name
        // to the container working directory, there is a high chance that
        // the full absolute path is over the limit. To work around this
        // limitation, we chdir first into the workdir where the socket is,
        // and chdir back after the socket is created.
        let workdir = socket_path
            .parent()
            .ok_or_else(|| NotifyListenerError::InvalidPath(socket_path.to_owned()))?;
        let socket_name = socket_path
            .file_name()
            .ok_or_else(|| NotifyListenerError::InvalidPath(socket_path.to_owned()))?;
        let cwd = env::current_dir().map_err(NotifyListenerError::GetCwd)?;
        unistd::chdir(workdir).map_err(|e| NotifyListenerError::Chdir {
            source: e,
            path: workdir.to_owned(),
        })?;
        let stream = UnixListener::bind(socket_name).map_err(|e| NotifyListenerError::Bind {
            source: e,
            name: socket_name.to_str().unwrap().to_owned(),
        })?;
        unistd::chdir(&cwd).map_err(|e| NotifyListenerError::Chdir {
            source: e,
            path: cwd,
        })?;

        Ok(Self { socket: stream })
    }

    pub fn wait_for_container_start(&self) -> Result<()> {
        match self.socket.accept() {
            Ok((mut socket, _)) => {
                let mut response = String::new();
                socket
                    .read_to_string(&mut response)
                    .map_err(NotifyListenerError::Read)?;
                log::debug!("received: {}", response);
            }
            Err(e) => Err(NotifyListenerError::Accept(e))?,
        }

        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        close(self.socket.as_raw_fd()).map_err(NotifyListenerError::Close)?;
        Ok(())
    }
}

pub struct NotifySocket {
    path: PathBuf,
}

impl NotifySocket {
    pub fn new<P: Into<PathBuf>>(socket_path: P) -> Self {
        Self {
            path: socket_path.into(),
        }
    }

    pub fn notify_container_start(&mut self) -> Result<()> {
        log::debug!("notify container start");
        let cwd = env::current_dir().map_err(NotifyListenerError::GetCwd)?;
        let workdir = self
            .path
            .parent()
            .ok_or_else(|| NotifyListenerError::InvalidPath(self.path.to_owned()))?;
        unistd::chdir(workdir).map_err(|e| NotifyListenerError::Chdir {
            source: e,
            path: workdir.to_owned(),
        })?;
        let socket_name = self
            .path
            .file_name()
            .ok_or_else(|| NotifyListenerError::InvalidPath(self.path.to_owned()))?;
        let mut stream =
            UnixStream::connect(socket_name).map_err(|e| NotifyListenerError::Connect {
                source: e,
                name: socket_name.to_str().unwrap().to_owned(),
            })?;
        stream
            .write_all(b"start container")
            .map_err(NotifyListenerError::SendStartContainer)?;
        log::debug!("notify finished");
        unistd::chdir(&cwd).map_err(|e| NotifyListenerError::Chdir {
            source: e,
            path: cwd,
        })?;
        Ok(())
    }
}
