use std::{any::Any, path::Path};

use anyhow::Result;
use caps::{errors::CapsError, CapSet, CapsHashSet};
use nix::{
    sched::CloneFlags,
    unistd::{Gid, Uid},
};

use crate::spec::LinuxRlimit;

pub trait Command {
    fn as_any(&self) -> &dyn Any;
    fn pivot_rootfs(&self, path: &Path) -> Result<()>;
    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()>;
    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()>;
    fn unshare(&self, flags: CloneFlags) -> Result<()>;
    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<(), CapsError>;
    fn set_hostname(&self, hostname: &str) -> Result<()>;
    fn set_rlimit(&self, rlimit: &LinuxRlimit) -> Result<()>;
}
