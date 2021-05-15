use std::{any::Any, path::Path};

use anyhow::{bail, Result};
use caps::{errors::CapsError, CapSet, CapsHashSet};
use nix::unistd::{fchdir, pivot_root, sethostname};
use nix::{fcntl::open, sched::CloneFlags};
use nix::{
    fcntl::OFlag,
    unistd::{Gid, Uid},
};
use nix::{
    mount::{umount2, MntFlags},
    unistd,
};
use nix::{sched::unshare, sys::stat::Mode};

use super::Command;
use crate::capabilities;

#[derive(Clone)]
pub struct LinuxCommand;

impl Command for LinuxCommand {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn pivot_rootfs(&self, path: &Path) -> Result<()> {
        let newroot = open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;

        pivot_root(path, path)?;

        umount2("/", MntFlags::MNT_DETACH)?;
        fchdir(newroot)?;
        Ok(())
    }

    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()> {
        nix::sched::setns(rawfd, nstype)?;
        Ok(())
    }

    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()> {
        if let Err(e) = prctl::set_keep_capabilities(true) {
            bail!("set keep capabilities returned {}", e);
        };
        unistd::setresgid(gid, gid, gid)?;
        unistd::setresuid(uid, uid, uid)?;

        if uid != Uid::from_raw(0) {
            capabilities::reset_effective(self)?;
        }
        if let Err(e) = prctl::set_keep_capabilities(false) {
            bail!("set keep capabilities returned {}", e);
        };
        Ok(())
    }

    fn unshare(&self, flags: CloneFlags) -> Result<()> {
        unshare(flags)?;
        Ok(())
    }

    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<(), CapsError> {
        caps::set(None, cset, value)
    }

    fn set_hostname(&self, hostname: &str) -> Result<()> {
        if let Err(e) = sethostname(hostname) {
            bail!("Failed to set {} as hostname. {:?}", hostname, e)
        }
        Ok(())
    }
}
