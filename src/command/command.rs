use std::path::Path;

use anyhow::Result;
use nix::{
    sched::CloneFlags,
    unistd::{Gid, Uid},
};

pub trait Command {
    fn pivot_rootfs(&self, path: &Path) -> Result<()>;
    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()>;
    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()>;
    fn unshare(&self, flags: CloneFlags) -> Result<()>;
}
