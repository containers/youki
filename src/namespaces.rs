//! Namespaces provide isolation of resources for processes at a kernel level.
//! The namespaces are: Mount (filesystem),
//! Process (processes in a namespace have two PIDs, one for the global PID,
//! which is used by the main system and the second one is for the child within the process tree),
//! Interprocess Communication (Control or communication between processes),
//! Network (which network devices can be seen by the processes in the namespace), User (User configs),
//! UTS (hostname and domain information, processes will think they're running on servers with different names),
//! Cgroup (Resource limits, execution priority etc.)

use crate::syscall::{syscall::create_syscall, Syscall};
use anyhow::{Context, Result};
use nix::{fcntl, sched::CloneFlags, sys::stat, unistd};
use oci_spec::{LinuxNamespace, LinuxNamespaceType};
use std::collections;

/// Holds information about namespaces
pub struct Namespaces {
    command: Box<dyn Syscall>,
    namespace_map: collections::HashMap<CloneFlags, LinuxNamespace>,
}

fn get_clone_flag(namespace_type: LinuxNamespaceType) -> CloneFlags {
    match namespace_type {
        LinuxNamespaceType::Pid => CloneFlags::CLONE_NEWPID,
        LinuxNamespaceType::User => CloneFlags::CLONE_NEWUSER,
        LinuxNamespaceType::Uts => CloneFlags::CLONE_NEWUTS,
        LinuxNamespaceType::Cgroup => CloneFlags::CLONE_NEWCGROUP,
        LinuxNamespaceType::Ipc => CloneFlags::CLONE_NEWIPC,
        LinuxNamespaceType::Network => CloneFlags::CLONE_NEWNET,
        LinuxNamespaceType::Mount => CloneFlags::CLONE_NEWNS,
    }
}

impl From<Option<&Vec<LinuxNamespace>>> for Namespaces {
    fn from(namespaces: Option<&Vec<LinuxNamespace>>) -> Self {
        let command: Box<dyn Syscall> = create_syscall();
        let namespace_map: collections::HashMap<CloneFlags, LinuxNamespace> = namespaces
            .unwrap_or(&vec![])
            .iter()
            .map(|ns| (get_clone_flag(ns.typ), ns.clone()))
            .collect();

        Namespaces {
            command,
            namespace_map,
        }
    }
}

impl Namespaces {
    pub fn apply_namespaces<F: Fn(CloneFlags) -> bool>(&self, filter: F) -> Result<()> {
        let to_enter: collections::HashMap<&CloneFlags, &LinuxNamespace> = self
            .namespace_map
            .iter()
            .filter(|(k, _)| filter(**k))
            .collect();
        for (ns_type, ns) in to_enter {
            self.unshare_or_setns(ns)
                .with_context(|| format!("Failed to enter {:?} namespace: {:?}", ns_type, ns))?;
        }
        Ok(())
    }

    pub fn unshare_or_setns(&self, namespace: &LinuxNamespace) -> Result<()> {
        log::debug!("unshare or setns: {:?}", namespace);
        if namespace.path.is_none() {
            self.command.unshare(get_clone_flag(namespace.typ))?;
        } else {
            let ns_path = namespace.path.as_ref().unwrap();
            let fd = fcntl::open(ns_path, fcntl::OFlag::empty(), stat::Mode::empty())
                .with_context(|| format!("Failed to open namespace fd: {:?}", ns_path))?;
            self.command
                .set_ns(fd, get_clone_flag(namespace.typ))
                .with_context(|| "Failed to set namespace")?;
            unistd::close(fd).with_context(|| "Failed to close namespace fd")?;
        }

        Ok(())
    }

    pub fn get(&self, k: LinuxNamespaceType) -> Option<&LinuxNamespace> {
        self.namespace_map.get(&get_clone_flag(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::test::TestHelperSyscall;
    use oci_spec::LinuxNamespaceType;

    fn gen_sample_linux_namespaces() -> Vec<LinuxNamespace> {
        vec![
            LinuxNamespace {
                typ: LinuxNamespaceType::Mount,
                path: Some("/dev/null".into()),
            },
            LinuxNamespace {
                typ: LinuxNamespaceType::Network,
                path: Some("/dev/null".into()),
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
    fn test_apply_namespaces() {
        let sample_linux_namespaces = gen_sample_linux_namespaces();
        let namespaces = Namespaces::from(Some(&sample_linux_namespaces));
        let test_command: &TestHelperSyscall = namespaces.command.as_any().downcast_ref().unwrap();
        assert!(namespaces
            .apply_namespaces(|ns_type| { ns_type != CloneFlags::CLONE_NEWIPC })
            .is_ok());

        let mut setns_args: Vec<_> = test_command
            .get_setns_args()
            .into_iter()
            .map(|(_fd, cf)| cf)
            .collect();
        setns_args.sort();
        let mut expect = vec![CloneFlags::CLONE_NEWNS, CloneFlags::CLONE_NEWNET];
        expect.sort();
        assert_eq!(setns_args, expect);

        let mut unshare_args = test_command.get_unshare_args();
        unshare_args.sort();
        let mut expect = vec![CloneFlags::CLONE_NEWUSER, CloneFlags::CLONE_NEWPID];
        expect.sort();
        assert_eq!(unshare_args, expect)
    }
}
