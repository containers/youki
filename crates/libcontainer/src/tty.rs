//! tty (teletype) for user-system interaction

use nix::errno::Errno;
use nix::sys::socket::{self, UnixAddr};
use nix::unistd::close;
use nix::unistd::dup2;
use std::io::IoSlice;
use std::os::unix::fs::symlink;
use std::os::unix::io::AsRawFd;
use std::os::unix::prelude::RawFd;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StdIO {
    Stdin = 0,
    Stdout = 1,
    Stderr = 2,
}

impl From<StdIO> for i32 {
    fn from(value: StdIO) -> Self {
        match value {
            StdIO::Stdin => 0,
            StdIO::Stdout => 1,
            StdIO::Stderr => 2,
        }
    }
}

impl std::fmt::Display for StdIO {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StdIO::Stdin => write!(f, "stdin"),
            StdIO::Stdout => write!(f, "stdout"),
            StdIO::Stderr => write!(f, "stderr"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TTYError {
    #[error("failed to connect/duplicate {stdio}")]
    ConnectStdIO { source: nix::Error, stdio: StdIO },
    #[error("failed to create console socket")]
    CreateConsoleSocket {
        source: nix::Error,
        socket_name: String,
    },
    #[error("failed to symlink console socket into container_dir")]
    Symlink {
        source: std::io::Error,
        linked: Box<PathBuf>,
        console_socket_path: Box<PathBuf>,
    },
    #[error("invalid socker name: {socket_name:?}")]
    InvalidSocketName {
        socket_name: String,
        source: nix::Error,
    },
    #[error("failed to create console socket fd")]
    CreateConsoleSocketFd { source: nix::Error },
    #[error("could not create pseudo terminal")]
    CreatePseudoTerminal { source: nix::Error },
    #[error("failed to send pty master")]
    SendPtyMaster { source: nix::Error },
    #[error("could not close console socket")]
    CloseConsoleSocket { source: nix::Error },
}

type Result<T> = std::result::Result<T, TTYError>;

// TODO: Handling when there isn't console-socket.
pub fn setup_console_socket(
    container_dir: &Path,
    console_socket_path: &Path,
    socket_name: &str,
) -> Result<RawFd> {
    let linked = container_dir.join(socket_name);
    symlink(console_socket_path, &linked).map_err(|err| TTYError::Symlink {
        source: err,
        linked: linked.to_path_buf().into(),
        console_socket_path: console_socket_path.to_path_buf().into(),
    })?;
    // Using ManuallyDrop to keep the socket open.
    let csocketfd = std::mem::ManuallyDrop::new(
        socket::socket(
            socket::AddressFamily::Unix,
            socket::SockType::Stream,
            socket::SockFlag::empty(),
            None,
        )
        .map_err(|err| TTYError::CreateConsoleSocketFd { source: err })?,
    );
    let csocketfd = match socket::connect(
        csocketfd.as_raw_fd(),
        &socket::UnixAddr::new(socket_name).map_err(|err| TTYError::InvalidSocketName {
            source: err,
            socket_name: socket_name.to_string(),
        })?,
    ) {
        Err(Errno::ENOENT) => -1,
        Err(errno) => Err(TTYError::CreateConsoleSocket {
            source: errno,
            socket_name: socket_name.to_string(),
        })?,
        Ok(()) => csocketfd.as_raw_fd(),
    };
    Ok(csocketfd)
}

pub fn setup_console(console_fd: &RawFd) -> Result<()> {
    // You can also access pty master, but it is better to use the API.
    // ref. https://github.com/containerd/containerd/blob/261c107ffc4ff681bc73988f64e3f60c32233b37/vendor/github.com/containerd/go-runc/console.go#L139-L154
    let openpty_result = nix::pty::openpty(None, None)
        .map_err(|err| TTYError::CreatePseudoTerminal { source: err })?;
    let pty_name: &[u8] = b"/dev/ptmx";
    let iov = [IoSlice::new(pty_name)];

    let [master, slave] = [openpty_result.master, openpty_result.slave];
    // Use ManuallyDrop to keep FDs open.
    let master = std::mem::ManuallyDrop::new(master);
    let slave = std::mem::ManuallyDrop::new(slave);

    let fds = [master.as_raw_fd()];
    let cmsg = socket::ControlMessage::ScmRights(&fds);
    socket::sendmsg::<UnixAddr>(
        console_fd.as_raw_fd(),
        &iov,
        &[cmsg],
        socket::MsgFlags::empty(),
        None,
    )
    .map_err(|err| TTYError::SendPtyMaster { source: err })?;

    if unsafe { libc::ioctl(slave.as_raw_fd(), libc::TIOCSCTTY) } < 0 {
        tracing::warn!("could not TIOCSCTTY");
    };
    let slave = slave.as_raw_fd();
    connect_stdio(&slave, &slave, &slave)?;
    close(console_fd.as_raw_fd()).map_err(|err| TTYError::CloseConsoleSocket { source: err })?;

    Ok(())
}

fn connect_stdio(stdin: &RawFd, stdout: &RawFd, stderr: &RawFd) -> Result<()> {
    dup2(stdin.as_raw_fd(), StdIO::Stdin.into()).map_err(|err| TTYError::ConnectStdIO {
        source: err,
        stdio: StdIO::Stdin,
    })?;
    dup2(stdout.as_raw_fd(), StdIO::Stdout.into()).map_err(|err| TTYError::ConnectStdIO {
        source: err,
        stdio: StdIO::Stdout,
    })?;
    // FIXME: Rarely does it fail.
    // error message: `Error: Resource temporarily unavailable (os error 11)`
    dup2(stderr.as_raw_fd(), StdIO::Stderr.into()).map_err(|err| TTYError::ConnectStdIO {
        source: err,
        stdio: StdIO::Stderr,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use serial_test::serial;
    use std::env;
    use std::fs::{self, File};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    const CONSOLE_SOCKET: &str = "console-socket";

    fn setup() -> Result<(tempfile::TempDir, PathBuf, PathBuf)> {
        let testdir = tempfile::tempdir()?;
        let rundir_path = Path::join(testdir.path(), "run");
        fs::create_dir(&rundir_path)?;
        let socket_path = Path::new(&rundir_path).join("socket");
        let _ = File::create(&socket_path);
        env::set_current_dir(&testdir)?;
        Ok((testdir, rundir_path, socket_path))
    }

    #[test]
    #[serial]
    fn test_setup_console_socket() {
        let init = setup();
        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let lis = UnixListener::bind(Path::join(testdir.path(), "console-socket"));
        assert!(lis.is_ok());
        let fd = setup_console_socket(&rundir_path, &socket_path, CONSOLE_SOCKET);
        assert!(fd.is_ok());
        assert_ne!(fd.unwrap().as_raw_fd(), -1);
    }

    #[test]
    #[serial]
    fn test_setup_console_socket_empty() {
        let init = setup();
        assert!(init.is_ok());
        let (_testdir, rundir_path, socket_path) = init.unwrap();
        let fd = setup_console_socket(&rundir_path, &socket_path, CONSOLE_SOCKET);
        assert!(fd.is_ok());
        assert_eq!(fd.unwrap().as_raw_fd(), -1);
    }

    #[test]
    #[serial]
    fn test_setup_console_socket_invalid() {
        let init = setup();
        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let _socket = File::create(Path::join(testdir.path(), "console-socket"));
        assert!(_socket.is_ok());
        let fd = setup_console_socket(&rundir_path, &socket_path, CONSOLE_SOCKET);
        assert!(fd.is_err());
    }

    #[test]
    #[serial]
    fn test_setup_console() {
        let init = setup();
        // duplicate the existing std* fds
        // we need to restore them later, and we cannot simply store them
        // as they themselves get modified in setup_console
        let old_stdin: RawFd = nix::unistd::dup(StdIO::Stdin.into()).unwrap();
        let old_stdout: RawFd = nix::unistd::dup(StdIO::Stdout.into()).unwrap();
        let old_stderr: RawFd = nix::unistd::dup(StdIO::Stderr.into()).unwrap();

        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let lis = UnixListener::bind(Path::join(testdir.path(), "console-socket"));
        assert!(lis.is_ok());
        let fd = setup_console_socket(&rundir_path, &socket_path, CONSOLE_SOCKET);
        let status = setup_console(&fd.unwrap());

        // restore the original std* before doing final assert
        dup2(old_stdin, StdIO::Stdin.into()).unwrap();
        dup2(old_stdout, StdIO::Stdout.into()).unwrap();
        dup2(old_stderr, StdIO::Stderr.into()).unwrap();

        assert!(status.is_ok());
    }
}
