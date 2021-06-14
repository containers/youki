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

pub fn setup_console(console_fd: FileDescriptor) -> Result<()> {
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
