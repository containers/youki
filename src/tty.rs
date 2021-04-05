use std::os::unix::fs::symlink;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use anyhow::{bail, Result};
use nix::errno::Errno;
use nix::fcntl;
use nix::sys::socket;
use nix::sys::stat;
use nix::unistd::{close, setsid};

use crate::stdio;
use crate::stdio::FileDescriptor;

pub fn ready(console_fd: FileDescriptor) -> Result<()> {
    let openpty_result = nix::pty::openpty(None, None)?;
    let data: &[u8] = b"/dev/ptmx";
    let iov = [nix::sys::uio::IoVec::from_slice(data)];
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

pub fn load_console_sockets(
    container_dir: &Path,
    console_socket: &str,
) -> Result<(FileDescriptor, FileDescriptor)> {
    let csocket = "console-stdout";
    symlink(console_socket, container_dir.join(csocket))?;

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
    let console = "console";
    let consolefd = match fcntl::open(
        &*console,
        fcntl::OFlag::O_NOCTTY | fcntl::OFlag::O_RDWR,
        stat::Mode::empty(),
    ) {
        Err(e) => {
            if e != ::nix::Error::Sys(Errno::ENOENT) {
                bail!("failed to open {}", console);
            }
            -1
        }
        Ok(fd) => fd,
    };
    Ok((csocketfd.into(), consolefd.into()))
}
