//! Unix pipe wrapper

use std::os::unix::io::RawFd;

use anyhow::Result;
use nix::fcntl::OFlag;
use nix::unistd::{close, pipe2, read};

pub struct Pipe {
    rfd: RawFd,
    wfd: RawFd,
}

impl Pipe {
    pub fn new() -> Result<Self> {
        // Sets as close-on-execution
        let (rfd, wfd) = pipe2(OFlag::O_CLOEXEC)?;
        Ok(Pipe { rfd, wfd })
    }

    pub fn wait(&self) -> Result<()> {
        close(self.wfd)?;
        let data: &mut [u8] = &mut [0];
        while read(self.rfd, data)? != 0 {}
        close(self.rfd)?;
        Ok(())
    }

    pub fn notify(&self) -> Result<()> {
        close(self.rfd)?;
        close(self.wfd)?;
        Ok(())
    }
}
