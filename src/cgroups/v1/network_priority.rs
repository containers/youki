use std::{fs::create_dir_all, path::Path};

use anyhow::Result;
use nix::unistd::Pid;

use crate::cgroups::common;
use crate::cgroups::v1::Controller;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkPriority {}

impl Controller for NetworkPriority {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");
        create_dir_all(&cgroup_root)?;

        if let Some(network) = linux_resources.network.as_ref() {
            Self::apply(cgroup_root, network)?;
        }

        common::write_cgroup_file(cgroup_root.join("cgroup.procs"), &pid.to_string())?;
        Ok(())
    }
}

impl NetworkPriority {
    fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        let priorities: String = network.priorities.iter().map(|p| p.to_string()).collect();
        common::write_cgroup_file(&root_path.join("net_prio.ifpriomap"), &priorities.trim())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::PathBuf};

    use super::*;
    use oci_spec::LinuxInterfacePriority;

    fn set_fixture(temp_dir: &std::path::Path, filename: &str, val: &str) -> Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(temp_dir.join(filename))?
            .write_all(val.as_bytes())?;

        Ok(())
    }

    fn create_temp_dir(test_name: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(std::env::temp_dir().join(test_name))?;
        Ok(std::env::temp_dir().join(test_name))
    }

    #[test]
    fn test_apply_network_priorites() {
        let tmp = create_temp_dir("test_apply_network_priorites")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_prio.ifpriomap", "").expect("set fixture for priority map");
        let priorities = vec![
            LinuxInterfacePriority {
                name: "a".to_owned(),
                priority: 1,
            },
            LinuxInterfacePriority {
                name: "b".to_owned(),
                priority: 2,
            },
        ];
        let priorities_string = priorities.iter().map(|p| p.to_string()).collect::<String>();
        let network = LinuxNetwork {
            class_id: None,
            priorities,
        };

        NetworkPriority::apply(&tmp, &network).expect("apply network priorities");

        let content =
            std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
        assert_eq!(priorities_string.trim(), content);
    }
}
