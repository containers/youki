use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::Arc,
};

// use debug_cell::{Ref, RefCell, RefMut};

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

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub enum ArgName {
    Namespace,
    Unshare,
    Mount,
    Symlink,
    Mknod,
    Chown,
    Hostname,
    Groups,
}

impl ArgName {
    fn iterator() -> impl Iterator<Item = ArgName> {
        [
            ArgName::Namespace,
            ArgName::Unshare,
            ArgName::Mount,
            ArgName::Symlink,
            ArgName::Mknod,
            ArgName::Chown,
            ArgName::Hostname,
            ArgName::Groups,
        ]
        .iter()
        .copied()
    }
}

struct MockCalls {
    args: HashMap<ArgName, RefCell<Mock>>,
}

impl Default for MockCalls {
    fn default() -> Self {
        let mut m = MockCalls {
            args: HashMap::new(),
        };

        for name in ArgName::iterator() {
            m.args.insert(name, RefCell::new(Mock::default()));
        }

        m
    }
}

impl MockCalls {
    fn act(&self, name: ArgName, value: Box<dyn Any>) -> anyhow::Result<()> {
        if self.args.get(&name).unwrap().borrow().ret_err_times > 0 {
            self.args.get(&name).unwrap().borrow_mut().ret_err_times -= 1;
            if let Some(e) = &self.args.get(&name).unwrap().borrow().ret_err {
                return e();
            }
        }

        self.args
            .get(&name)
            .unwrap()
            .borrow_mut()
            .values
            .push(value);
        Ok(())
    }

    fn fetch(&self, name: ArgName) -> Ref<Mock> {
        self.args.get(&name).unwrap().borrow()
    }

    fn fetch_mut(&self, name: ArgName) -> RefMut<Mock> {
        self.args.get(&name).unwrap().borrow_mut()
    }
}

pub struct TestHelperSyscall {
    mocks: MockCalls,
    set_capability_args: RefCell<Vec<(CapSet, CapsHashSet)>>,

    set_ns_args: RefCell<Vec<(i32, CloneFlags)>>,
    unshare_args: RefCell<Vec<CloneFlags>>,
    symlink_args: RefCell<Vec<(PathBuf, PathBuf)>>,
    mknod_args: RefCell<Vec<MknodArgs>>,
    chown_args: RefCell<Vec<ChownArgs>>,
    hostname_args: RefCell<Vec<String>>,
    groups_args: RefCell<Vec<Vec<Gid>>>,
}

impl Default for TestHelperSyscall {
    fn default() -> Self {
        TestHelperSyscall {
            mocks: MockCalls::default(),
            set_ns_args: RefCell::new(vec![]),
            unshare_args: RefCell::new(vec![]),
            set_capability_args: RefCell::new(vec![]),
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
        let v = MountArgs {
            source: source.map(|x| x.to_owned()),
            target: target.to_owned(),
            fstype: fstype.map(|x| x.to_owned()),
            flags,
            data: data.map(|x| x.to_owned()),
        };
        self.mocks.act(ArgName::Mount, Box::new(v))
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
    pub fn set_ret_err(&self, name: ArgName, err: fn() -> anyhow::Result<()>) {
        self.mocks.fetch_mut(name).ret_err = Some(err);
        self.set_ret_err_times(name, 1);
    }

    pub fn set_ret_err_times(&self, name: ArgName, times: usize) {
        self.mocks.fetch_mut(name).ret_err_times = times;
    }

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
        self.mocks
            .fetch(ArgName::Mount)
            .values
            .iter()
            .map(|x| x.downcast_ref::<MountArgs>().unwrap().clone())
            .collect::<Vec<MountArgs>>()
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
