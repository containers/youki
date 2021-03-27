use std::os::unix::io::{AsRawFd, RawFd};

use anyhow::Result;
use nix::unistd::dup2;

#[derive(Debug)]
pub struct FileDescriptor(RawFd);

const STDIN: i32 = 0;
const STDOUT: i32 = 1;
const STDERR: i32 = 2;

// impl Drop for FileDescriptor {
//     fn drop(&mut self) {
//         close(self.0).expect("FileDescriptor close failed.")
//     }
// }

impl AsRawFd for FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl From<u8> for FileDescriptor {
    fn from(rawfd: u8) -> Self {
        FileDescriptor(RawFd::from(rawfd))
    }
}

impl From<RawFd> for FileDescriptor {
    fn from(fd: RawFd) -> Self {
        FileDescriptor(fd)
    }
}

pub fn connect_stdio(
    stdin: &FileDescriptor,
    stdout: &FileDescriptor,
    stderr: &FileDescriptor,
) -> Result<()> {
    std::thread::sleep(std::time::Duration::from_millis(10));
    dup2(stdin.as_raw_fd(), STDIN)?;
    dup2(stdout.as_raw_fd(), STDOUT)?;
    // FIXME: Rarely does it fail.
    // error message: `Error: Resource temporarily unavailable (os error 11)`
    dup2(stderr.as_raw_fd(), STDERR)?;
    Ok(())
}
