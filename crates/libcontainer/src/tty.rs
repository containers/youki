//! tty (teletype) for user-system interaction

use std::io::IoSlice;
use std::os::unix::fs::symlink;
use std::os::unix::io::AsRawFd;
use std::os::unix::prelude::RawFd;
use std::path::Path;

use anyhow::Context;
use anyhow::{bail, Result};
use nix::errno::Errno;
use nix::sys::socket::{self, UnixAddr};
use nix::unistd::close;
use nix::unistd::dup2;

const STDIN: i32 = 0;
const STDOUT: i32 = 1;
const STDERR: i32 = 2;

// TODO: Handling when there isn't console-socket.
pub fn setup_console_socket(
    container_dir: &Path,
    console_socket_path: &Path,
    socket_name: &str,
) -> Result<RawFd> {
    let linked = container_dir.join(socket_name);
    symlink(console_socket_path, linked)?;

    let mut csocketfd = socket::socket(
        socket::AddressFamily::Unix,
        socket::SockType::Stream,
        socket::SockFlag::empty(),
        None,
    )?;
    csocketfd = match socket::connect(csocketfd, &socket::UnixAddr::new(socket_name)?) {
        Err(errno) => {
            if !matches!(errno, Errno::ENOENT) {
                bail!("failed to open {}", socket_name);
            }
            -1
        }
        Ok(()) => csocketfd,
    };
    Ok(csocketfd)
}

pub fn setup_console(console_fd: &RawFd) -> Result<()> {
    // You can also access pty master, but it is better to use the API.
    // ref. https://github.com/containerd/containerd/blob/261c107ffc4ff681bc73988f64e3f60c32233b37/vendor/github.com/containerd/go-runc/console.go#L139-L154
    let openpty_result =
        nix::pty::openpty(None, None).context("could not create pseudo terminal")?;
    let pty_name: &[u8] = b"/dev/ptmx";
    let iov = [IoSlice::new(pty_name)];
    let fds = [openpty_result.master];
    let cmsg = socket::ControlMessage::ScmRights(&fds);
    socket::sendmsg::<UnixAddr>(
        console_fd.as_raw_fd(),
        &iov,
        &[cmsg],
        socket::MsgFlags::empty(),
        None,
    )
    .context("failed to send pty master")?;

    if unsafe { libc::ioctl(openpty_result.slave, libc::TIOCSCTTY) } < 0 {
        log::warn!("could not TIOCSCTTY");
    };
    let slave = openpty_result.slave;
    connect_stdio(&slave, &slave, &slave).context("could not dup tty to stderr")?;
    close(console_fd.as_raw_fd()).context("could not close console socket")?;
    Ok(())
}

fn connect_stdio(stdin: &RawFd, stdout: &RawFd, stderr: &RawFd) -> Result<()> {
    dup2(stdin.as_raw_fd(), STDIN)?;
    dup2(stdout.as_raw_fd(), STDOUT)?;
    // FIXME: Rarely does it fail.
    // error message: `Error: Resource temporarily unavailable (os error 11)`
    dup2(stderr.as_raw_fd(), STDERR)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use std::fs::{self, File};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    use serial_test::serial;

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
        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let lis = UnixListener::bind(Path::join(testdir.path(), "console-socket"));
        assert!(lis.is_ok());
        let fd = setup_console_socket(&rundir_path, &socket_path, CONSOLE_SOCKET);
        let status = setup_console(&fd.unwrap());
        assert!(status.is_ok());
    }
}
