use anyhow::{bail, Context, Result};
use nix::unistd::{self, close};
use std::env;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub const NOTIFY_FILE: &str = "notify.sock";

pub struct NotifyListener {
    socket: UnixListener,
}

impl NotifyListener {
    pub fn new(socket_path: &Path) -> Result<Self> {
        // unix domain socket has a maximum length of 108, different from
        // normal path length of 255. due to how docker create the path name
        // to the container working directory, there is a high chance that
        // the full absolute path is over the limit. to work around this
        // limitation, we chdir first into the workdir where the socket is,
        // and chdir back after the socket is created.
        let workdir = socket_path.parent().unwrap();
        let socket_name = socket_path.file_name().unwrap();
        let cwd = unistd::getcwd().context("Failed to get cwd")?;
        unistd::chdir(workdir).context(format!(
            "Failed to chdir into {}",
            workdir.to_str().unwrap()
        ))?;
        let stream = UnixListener::bind(socket_name)
            .context(format!("Failed to bind {}", socket_name.to_str().unwrap()))?;
        unistd::chdir(&cwd)
            .context(format!("Failed to chdir back to {}", cwd.to_str().unwrap()))?;

        Ok(Self { socket: stream })
    }

    pub fn wait_for_container_start(&self) -> Result<()> {
        match self.socket.accept() {
            Ok((mut socket, _)) => {
                let mut response = String::new();
                socket.read_to_string(&mut response)?;
                log::debug!("received: {}", response);
            }
            Err(e) => bail!("accept function failed: {:?}", e),
        }

        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        close(self.socket.as_raw_fd())?;
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
        let cwd = env::current_dir()?;
        unistd::chdir(&*self.path.parent().unwrap())?;
        let mut stream = UnixStream::connect(&self.path.file_name().unwrap())?;
        stream.write_all(b"start container")?;
        log::debug!("notify finished");
        unistd::chdir(&*cwd)?;
        Ok(())
    }

    pub fn notify_container_finish(&mut self) -> Result<()> {
        // self.socket.write_all(b"finish container")?;
        Ok(())
    }
}
