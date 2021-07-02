//! tty (teletype) for user-system interaction

use std::os::unix::fs::symlink;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use anyhow::{bail, Result};
use nix::errno::Errno;
use nix::sys::socket;
use nix::sys::uio;
use nix::unistd::{close, setsid};

use crate::stdio;
use crate::stdio::FileDescriptor;

// TODO: Handling when there isn't console-socket.

pub fn setup_console_socket(
    container_dir: &Path,
    console_socket_path: &Path,
) -> Result<FileDescriptor> {
    let csocket = "console-socket";
    symlink(console_socket_path, container_dir.join(csocket))?;

    let mut csocketfd = socket::socket(
        socket::AddressFamily::Unix,
        socket::SockType::Stream,
        socket::SockFlag::empty(),
        None,
    )?;
    csocketfd = match socket::connect(
        csocketfd,
        &socket::SockAddr::Unix(socket::UnixAddr::new(&*csocket)?),
    ) {
        Err(e) => {
            if e != ::nix::Error::Sys(Errno::ENOENT) {
                bail!("failed to open {}", csocket);
            }
            -1
        }
        Ok(()) => csocketfd,
    };
    Ok(csocketfd.into())
}

pub fn setup_console(console_fd: &FileDescriptor) -> Result<()> {
    // You can also access pty master, but it is better to use the API.
    // ref. https://github.com/containerd/containerd/blob/261c107ffc4ff681bc73988f64e3f60c32233b37/vendor/github.com/containerd/go-runc/console.go#L139-L154
    let openpty_result = nix::pty::openpty(None, None)?;
    let pty_name: &[u8] = b"/dev/ptmx";
    let iov = [uio::IoVec::from_slice(pty_name)];
    let fds = [openpty_result.master];
    let cmsg = socket::ControlMessage::ScmRights(&fds);
    socket::sendmsg(
        console_fd.as_raw_fd(),
        &iov,
        &[cmsg],
        socket::MsgFlags::empty(),
        None,
    )?;

    setsid()?;
    if unsafe { libc::ioctl(openpty_result.slave, libc::TIOCSCTTY) } < 0 {
        log::warn!("could not TIOCSCTTY");
    };
    let slave = FileDescriptor::from(openpty_result.slave);
    stdio::connect_stdio(&slave, &slave, &slave).expect("could not dup tty to stderr");
    close(console_fd.as_raw_fd())?;
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

    use crate::utils::{create_temp_dir, TempDir};

    fn setup(testname: &str) -> Result<(TempDir, PathBuf, PathBuf)> {
        let testdir = create_temp_dir(testname)?;
        let rundir_path = Path::join(&testdir, "run");
        let _ = fs::create_dir(&rundir_path)?;
        let socket_path = Path::new(&rundir_path).join("socket");
        let _ = File::create(&socket_path);
        env::set_current_dir(&testdir)?;
        Ok((testdir, rundir_path, socket_path))
    }

    #[test]
    #[serial]
    fn test_setup_console_socket() {
        let init = setup("test_setup_console_socket");
        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let lis = UnixListener::bind(Path::join(&testdir, "console-socket"));
        assert!(lis.is_ok());
        let fd = setup_console_socket(&&rundir_path, &socket_path);
        assert!(fd.is_ok());
        assert_ne!(fd.unwrap().as_raw_fd(), -1);
    }

    #[test]
    #[serial]
    fn test_setup_console_socket_empty() {
        let init = setup("test_setup_console_socket_empty");
        assert!(init.is_ok());
        let (_testdir, rundir_path, socket_path) = init.unwrap();
        let fd = setup_console_socket(&rundir_path, &socket_path);
        assert!(fd.is_ok());
        assert_eq!(fd.unwrap().as_raw_fd(), -1);
    }

    #[test]
    #[serial]
    fn test_setup_console_socket_invalid() {
        let init = setup("test_setup_console_socket_invalid");
        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let _socket = File::create(Path::join(&testdir, "console-socket"));
        assert!(_socket.is_ok());
        let fd = setup_console_socket(&rundir_path, &socket_path);
        assert!(fd.is_err());
    }

    #[test]
    #[serial]
    fn test_setup_console() {
        let init = setup("test_setup_console");
        assert!(init.is_ok());
        let (testdir, rundir_path, socket_path) = init.unwrap();
        let lis = UnixListener::bind(Path::join(&testdir, "console-socket"));
        assert!(lis.is_ok());
        let fd = setup_console_socket(&&rundir_path, &socket_path);
        let status = setup_console(&fd.unwrap());
        assert!(status.is_ok());
    }
}
