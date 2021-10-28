use std::{
    any::Any,
    cell::RefCell,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::Arc,
};

use caps::{errors::CapsError, CapSet, CapsHashSet};
use nix::{
    mount::MsFlags,
    sched::CloneFlags,
    sys::stat::{Mode, SFlag},
    unistd::{Gid, Uid},
};

use oci_spec::runtime::LinuxRlimit;

use super::Syscall;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MountArgs {
    pub source: Option<PathBuf>,
    pub target: PathBuf,
    pub fstype: Option<String>,
    pub flags: MsFlags,
    pub data: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MknodArgs {
    pub path: PathBuf,
    pub kind: SFlag,
    pub perm: Mode,
    pub dev: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChownArgs {
    pub path: PathBuf,
    pub owner: Option<Uid>,
    pub group: Option<Gid>,
}

#[derive(Default)]
struct Mock {
    values: Vec<Box<dyn Any>>,
    ret_err: Option<fn() -> anyhow::Result<()>>,
    ret_err_times: usize,
}

// #[derive(Clone)]
pub struct TestHelperSyscall {
    set_ns_args: RefCell<Vec<(i32, CloneFlags)>>,
    unshare_args: RefCell<Vec<CloneFlags>>,
    set_capability_args: RefCell<Vec<(CapSet, CapsHashSet)>>,
    mount_args: RefCell<Mock>,
    symlink_args: RefCell<Vec<(PathBuf, PathBuf)>>,
    mknod_args: RefCell<Vec<MknodArgs>>,
    chown_args: RefCell<Vec<ChownArgs>>,
    hostname_args: RefCell<Vec<String>>,
    groups_args: RefCell<Vec<Vec<Gid>>>,
}

impl Default for TestHelperSyscall {
    fn default() -> Self {
        TestHelperSyscall {
            set_ns_args: RefCell::new(vec![]),
            unshare_args: RefCell::new(vec![]),
            set_capability_args: RefCell::new(vec![]),
            mount_args: RefCell::new(Mock::default()),
            symlink_args: RefCell::new(vec![]),
            mknod_args: RefCell::new(vec![]),
            chown_args: RefCell::new(vec![]),
            hostname_args: RefCell::new(vec![]),
            groups_args: RefCell::new(vec![]),
        }
    }
}

impl Syscall for TestHelperSyscall {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn pivot_rootfs(&self, _path: &Path) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn set_ns(&self, rawfd: i32, nstype: CloneFlags) -> anyhow::Result<()> {
        let args = (rawfd, nstype);
        self.set_ns_args.borrow_mut().push(args);
        Ok(())
    }

    fn set_id(&self, _uid: Uid, _gid: Gid) -> anyhow::Result<()> {
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

    fn set_hostname(&self, hostname: &str) -> anyhow::Result<()> {
        self.hostname_args.borrow_mut().push(hostname.to_owned());
        Ok(())
    }

    fn set_rlimit(&self, _rlimit: &LinuxRlimit) -> anyhow::Result<()> {
        todo!()
    }

    fn get_pwuid(&self, _: u32) -> Option<Arc<OsStr>> {
        Some(OsString::from("youki").into())
    }

    fn chroot(&self, _: &Path) -> anyhow::Result<()> {
        todo!()
    }

    fn mount(
        &self,
        source: Option<&Path>,
        target: &Path,
        fstype: Option<&str>,
        flags: MsFlags,
        data: Option<&str>,
    ) -> anyhow::Result<()> {
        if self.mount_args.borrow().ret_err_times > 0 {
            self.mount_args.borrow_mut().ret_err_times -= 1;
            if let Some(e) = &self.mount_args.borrow().ret_err {
                return e();
            }
        }

        let v = MountArgs {
            source: source.map(|x| x.to_owned()),
            target: target.to_owned(),
            fstype: fstype.map(|x| x.to_owned()),
            flags,
            data: data.map(|x| x.to_owned()),
        };
        self.mount_args.borrow_mut().values.push(Box::new(v));
        Ok(())
    }

    fn symlink(&self, original: &Path, link: &Path) -> anyhow::Result<()> {
        self.symlink_args
            .borrow_mut()
            .push((original.to_path_buf(), link.to_path_buf()));
        Ok(())
    }

    fn mknod(&self, path: &Path, kind: SFlag, perm: Mode, dev: u64) -> anyhow::Result<()> {
        self.mknod_args.borrow_mut().push(MknodArgs {
            path: path.to_path_buf(),
            kind,
            perm,
            dev,
        });
        Ok(())
    }
    fn chown(&self, path: &Path, owner: Option<Uid>, group: Option<Gid>) -> anyhow::Result<()> {
        self.chown_args.borrow_mut().push(ChownArgs {
            path: path.to_path_buf(),
            owner,
            group,
        });
        Ok(())
    }

    fn set_groups(&self, groups: &[Gid]) -> anyhow::Result<()> {
        self.groups_args.borrow_mut().push(groups.to_vec());
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

    pub fn get_mount_args(&self) -> Vec<MountArgs> {
        self.mount_args
            .borrow()
            .values
            .iter()
            .map(|x| x.downcast_ref::<MountArgs>().unwrap().clone())
            .collect::<Vec<MountArgs>>()
    }

    pub fn set_mount_ret_err(&self, err: Option<fn() -> anyhow::Result<()>>, ret_times: usize) {
        self.mount_args.borrow_mut().ret_err = err;
        self.mount_args.borrow_mut().ret_err_times = ret_times;
    }

    pub fn get_symlink_args(&self) -> Vec<(PathBuf, PathBuf)> {
        self.symlink_args.borrow_mut().clone()
    }

    pub fn get_mknod_args(&self) -> Vec<MknodArgs> {
        self.mknod_args.borrow_mut().clone()
    }

    pub fn get_chown_args(&self) -> Vec<ChownArgs> {
        self.chown_args.borrow_mut().clone()
    }

    pub fn get_hostname_args(&self) -> Vec<String> {
        self.hostname_args.borrow_mut().clone()
    }

    pub fn get_groups_args(&self) -> Vec<Vec<Gid>> {
        self.groups_args.borrow_mut().clone()
    }
}
