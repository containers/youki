//! Namespaces provide isolation of resources for processes at a kernel level.
//! The namespaces are: Mount (filesystem),
//! Process (processes in a namespace have two PIDs, one for the global PID,
//! which is used by the main system and the second one is for the child within the process tree),
//! Interprocess Communication (Control or communication between processes),
//! Network (which network devices can be seen by the processes in the namespace), User (User configs),
//! UTS (hostname and domain information, processes will think they're running on servers with different names),
//! Cgroup (Resource limits, execution priority etc.)

use crate::syscall::{syscall::create_syscall, Syscall};
use anyhow::Result;
use nix::{
    fcntl,
    sched::{self, CloneFlags},
    sys::stat,
    unistd::{self, Gid, Uid},
};
use oci_spec::LinuxNamespace;

/// Holds information about namespaces
pub struct Namespaces {
    spaces: Vec<LinuxNamespace>,
    command: Box<dyn Syscall>,
    pub clone_flags: CloneFlags,
}

impl From<Vec<LinuxNamespace>> for Namespaces {
    fn from(namespaces: Vec<LinuxNamespace>) -> Self {
        let clone_flags = namespaces.iter().filter(|ns| ns.path.is_none()).fold(
            CloneFlags::empty(),
            |mut cf, ns| {
                cf |= CloneFlags::from_bits_truncate(ns.typ as i32);
                cf
            },
        );
        let command: Box<dyn Syscall> = create_syscall();

        Namespaces {
            spaces: namespaces,
            command,
            clone_flags,
        }
    }
}

impl Namespaces {
    /// sets namespaces as defined in structure to calling process
    pub fn apply_setns(&self) -> Result<()> {
        let to_enter: Vec<(CloneFlags, i32)> = self
            .spaces
            .iter()
            .filter(|ns| ns.path.is_some()) // filter those which are actually present on the system
            .map(|ns| {
                let space = CloneFlags::from_bits_truncate(ns.typ as i32);
                let fd = fcntl::open(
                    &*ns.path.as_ref().unwrap().clone(),
                    fcntl::OFlag::empty(),
                    stat::Mode::empty(),
                )
                .unwrap();
                (space, fd)
            })
            .collect();

        for &(space, fd) in &to_enter {
            // set the namespace
            self.command.set_ns(fd, space)?;
            unistd::close(fd)?;
            // if namespace is cloned with newuser flag, then it creates a new user namespace,
            // and we need to set the user and group id to 0
            // see https://man7.org/linux/man-pages/man2/clone.2.html for more info
            if space == sched::CloneFlags::CLONE_NEWUSER {
                self.command.set_id(Uid::from_raw(0), Gid::from_raw(0))?;
            }
        }
        Ok(())
    }

    /// disassociate given parts context of calling process from other process
    // see https://man7.org/linux/man-pages/man2/unshare.2.html for more info
    pub fn apply_unshare(&self, without: CloneFlags) -> Result<()> {
        self.command.unshare(self.clone_flags & !without)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use oci_spec::LinuxNamespaceType;

    use super::*;
    use crate::syscall::test::TestHelperSyscall;

    fn gen_sample_linux_namespaces() -> Vec<LinuxNamespace> {
        vec![
            LinuxNamespace {
                typ: LinuxNamespaceType::Mount,
                path: Some("/dev/null".to_string()),
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Network,
                path: Some("/dev/null".to_string()),
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Pid,
                path: None,
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::User,
                path: None,
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Ipc,
                path: None,
            },
        ]
    }

    #[test]
    fn test_namespaces_set_ns() {
        let sample_linux_namespaces = gen_sample_linux_namespaces();
        let namespaces: Namespaces = sample_linux_namespaces.into();
        let test_command: &TestHelperSyscall = namespaces.command.as_any().downcast_ref().unwrap();
        assert!(namespaces.apply_setns().is_ok());

        let mut setns_args: Vec<_> = test_command
            .get_setns_args()
            .into_iter()
            .map(|(_fd, cf)| cf)
            .collect();
        setns_args.sort();
        let mut expect = vec![CloneFlags::CLONE_NEWNS, CloneFlags::CLONE_NEWNET];
        expect.sort();
        assert_eq!(setns_args, expect);
    }

    #[test]
    fn test_namespaces_unshare() {
        let sample_linux_namespaces = gen_sample_linux_namespaces();
        let namespaces: Namespaces = sample_linux_namespaces.into();
        assert!(namespaces.apply_unshare(CloneFlags::CLONE_NEWIPC).is_ok());

        let test_command: &TestHelperSyscall = namespaces.command.as_any().downcast_ref().unwrap();
        let mut unshare_args = test_command.get_unshare_args();
        unshare_args.sort();
        let mut expect = vec![CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWPID];
        expect.sort();
        assert_eq!(unshare_args, expect)
    }
}
