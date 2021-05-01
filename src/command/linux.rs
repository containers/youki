use std::path::Path;

use anyhow::Result;
use nix::fcntl::open;
use nix::fcntl::OFlag;
use nix::mount::{umount2, MntFlags};
use nix::sys::stat::Mode;
use nix::unistd::{fchdir, pivot_root};

use super::Command;

pub struct LinuxCommand;

impl Command for LinuxCommand {
    fn pivot_rootfs(&self, path: &Path) -> Result<()> {
        let newroot = open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;

        pivot_root(path, path)?;

        umount2("/", MntFlags::MNT_DETACH)?;
        fchdir(newroot)?;
        Ok(())
    }
}
