use std::{any::Any, cell::RefCell, ffi::OsStr, sync::Arc};

use caps::{errors::CapsError, CapSet, CapsHashSet};
use nix::sched::CloneFlags;
use oci_spec::runtime::LinuxRlimit;

use super::Syscall;
use nix::mount::MsFlags;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct TestHelperSyscall {
    set_ns_args: RefCell<Vec<(i32, CloneFlags)>>,
    unshare_args: RefCell<Vec<CloneFlags>>,
    set_capability_args: RefCell<Vec<(CapSet, CapsHashSet)>>,
    mount_args: RefCell<Vec<MountArgs>>,
}

#[derive(Clone)]
pub struct MountArgs {
    source: Option<PathBuf>,
    target: PathBuf,
    fstype: Option<String>,
    flags: MsFlags,
    data: Option<String>,
}

impl Default for TestHelperSyscall {
    fn default() -> Self {
        TestHelperSyscall {
            set_ns_args: RefCell::new(vec![]),
            unshare_args: RefCell::new(vec![]),
            set_capability_args: RefCell::new(vec![]),
            mount_args: RefCell::new(vec![]),
        }
    }
}

impl Syscall for TestHelperSyscall {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn pivot_rootfs(&self, _path: &std::path::Path) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> anyhow::Result<()> {
        let args = (rawfd, nstype);
        self.set_ns_args.borrow_mut().push(args);
        Ok(())
    }

    fn set_id(&self, _uid: nix::unistd::Uid, _gid: nix::unistd::Gid) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn unshare(&self, flags: CloneFlags) -> anyhow::Result<()> {
        self.unshare_args.borrow_mut().push(flags);
        Ok(())
    }

    fn set_capability(&self, cset: CapSet, value: &CapsHashSet) -> Result<(), CapsError> {
        let args = (cset, value.clone());
        self.set_capability_args.borrow_mut().push(args);
        Ok(())
    }

    fn set_hostname(&self, _hostname: &str) -> anyhow::Result<()> {
        todo!()
    }

    fn set_rlimit(&self, _rlimit: &LinuxRlimit) -> anyhow::Result<()> {
        todo!()
    }

    fn get_pwuid(&self, _: u32) -> Option<Arc<OsStr>> {
        todo!()
    }

    fn chroot(&self, _: &std::path::Path) -> anyhow::Result<()> {
        todo!()
    }

    fn mount(
        &self,
        source: Option<&Path>,
        target: &Path,
        fstype: Option<&str>,
        flags: MsFlags,
        data: Option<&str>,
    ) -> Result<(), nix::errno::Errno> {
        let args = MountArgs {
            source: source.map(|x| x.to_owned()),
            target: target.to_owned(),
            fstype: fstype.map(|x| x.to_owned()),
            flags,
            data: data.map(|x| x.to_owned()),
        };
        self.mount_args.borrow_mut().push(args);
        Ok(())
    }
}

impl TestHelperSyscall {
    pub fn get_setns_args(&self) -> Vec<(i32, CloneFlags)> {
        self.set_ns_args.borrow_mut().clone()
    }

    pub fn get_unshare_args(&self) -> Vec<CloneFlags> {
        self.unshare_args.borrow_mut().clone()
    }

    pub fn get_set_capability_args(&self) -> Vec<(CapSet, CapsHashSet)> {
        self.set_capability_args.borrow_mut().clone()
    }
}
