//! An interface trait so that rest of Youki can call
//! necessary functions without having to worry about their
//! implementation details
use std::{any::Any, ffi::OsStr, path::Path, sync::Arc};

use anyhow::Result;
use bitflags::bitflags;
use caps::{CapSet, CapsHashSet};
use libc;
use nix::{
    mount::MsFlags,
    sched::CloneFlags,
    sys::stat::{Mode, SFlag},
    unistd::{Gid, Uid},
};

use oci_spec::runtime::LinuxRlimit;

use crate::syscall::{
    linux::{LinuxSyscall, MountAttr},
    test::TestHelperSyscall,
};

/// This specifies various kernel/other functionalities required for
/// container management
pub trait Syscall {
    fn as_any(&self) -> &dyn Any;
    fn pivot_rootfs(&self, path: &Path) -> Result<()>;
    fn chroot(&self, path: &Path) -> Result<()>;
    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> Result<()>;
    fn set_id(&self, uid: Uid, gid: Gid) -> Result<()>;
    fn unshare(&self, flags: CloneFlags) -> Result<()>;
    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<()>;
    fn set_hostname(&self, hostname: &str) -> Result<()>;
    fn set_domainname(&self, domainname: &str) -> Result<()>;
    fn set_rlimit(&self, rlimit: &LinuxRlimit) -> Result<()>;
    fn get_pwuid(&self, uid: u32) -> Option<Arc<OsStr>>;
    fn mount(
        &self,
        source: Option<&Path>,
        target: &Path,
        fstype: Option<&str>,
        flags: MsFlags,
        data: Option<&str>,
    ) -> Result<()>;
    fn symlink(&self, original: &Path, link: &Path) -> Result<()>;
    fn mknod(&self, path: &Path, kind: SFlag, perm: Mode, dev: u64) -> Result<()>;
    fn chown(&self, path: &Path, owner: Option<Uid>, group: Option<Gid>) -> Result<()>;
    fn set_groups(&self, groups: &[Gid]) -> Result<()>;
    fn close_range(&self, preserve_fds: i32) -> Result<()>;
    fn mount_setattr(
        &self,
        dirfd: i32,
        pathname: &Path,
        flags: u32,
        mount_attr: &MountAttr,
        size: libc::size_t,
    ) -> Result<()>;
}

pub fn create_syscall() -> Box<dyn Syscall> {
    if cfg!(test) {
        Box::new(TestHelperSyscall::default())
    } else {
        Box::new(LinuxSyscall)
    }
}

bitflags! {
pub struct CloseRange : usize {
    const NONE = 0b00000000;
    const UNSHARE = 0b00000010;
    const CLOEXEC = 0b00000100;
}}
