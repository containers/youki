//! Conditional variable that performs busy waiting on lock
//! and notifies when 

use std::os::unix::io::RawFd;

use anyhow::Result;
use nix::fcntl::OFlag;
use nix::unistd::{close, pipe2, read};

pub struct Cond {
    rfd: RawFd,
    wfd: RawFd,
}

impl Cond {
    pub fn new() -> Result<Cond> {
        let (rfd, wfd) = pipe2(OFlag::O_CLOEXEC)?; //Sets as close-on-execution
        Ok(Cond { rfd, wfd })
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
