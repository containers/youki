use std::os::unix::io::{AsRawFd, RawFd};

use anyhow::Result;
use nix::unistd::dup2;

const STDIN: i32 = 0;
const STDOUT: i32 = 1;
const STDERR: i32 = 2;

pub fn connect_stdio(stdin: &RawFd, stdout: &RawFd, stderr: &RawFd) -> Result<()> {
    dup2(stdin.as_raw_fd(), STDIN)?;
    dup2(stdout.as_raw_fd(), STDOUT)?;
    // FIXME: Rarely does it fail.
    // error message: `Error: Resource temporarily unavailable (os error 11)`
    dup2(stderr.as_raw_fd(), STDERR)?;
    Ok(())
}
