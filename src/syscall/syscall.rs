//! An interface trait so that rest of Youki can call
//! necessary functions without having to worry about their
//! implementation details
use std::{any::Any, ffi::OsStr, path::Path, sync::Arc};

use anyhow::Result;
use caps::{errors::CapsError, CapSet, CapsHashSet};
use nix::{
    mount::MsFlags,
    sched::CloneFlags,
    unistd::{Gid, Uid},
};

use oci_spec::runtime::LinuxRlimit;

use crate::syscall::{linux::LinuxSyscall, test::TestHelperSyscall};

/// This specifies various kernel/other functionalities required for
/// container management
pub trait Syscall {
    fn as_any(&self) -> &dyn Any;
    fn pivot_rootfs(&self, path: &Path) -> Result<()>;
    fn chroot(&self, path: &Path) -> Result<()>;
    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()>;
    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()>;
    fn unshare(&self, flags: CloneFlags) -> Result<()>;
    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<(), CapsError>;
    fn set_hostname(&self, hostname: &str) -> Result<()>;
    fn set_rlimit(&self, rlimit: &LinuxRlimit) -> Result<()>;
    fn get_pwuid(&self, uid: u32) -> Option<Arc<OsStr>>;
    // TODO: method signature (have to use a concrete type such as Path, but is it ok?
    fn mount(
        &self,
        // FIXME: this is not always path (such as 'tmpfs')...
        source: Option<&Path>,
        target: &Path,
        fstype: Option<&str>,
        flags: MsFlags,
        data: Option<&str>,
    ) -> Result<(), nix::errno::Errno>;
}

pub fn create_syscall() -> Box<dyn Syscall> {
    if cfg!(test) {
        Box::new(TestHelperSyscall::default())
    } else {
        Box::new(LinuxSyscall)
    }
}
