use std::path::Path;

use anyhow::{Context, Result};

use super::Controller;
use crate::common;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkPriority {}

impl Controller for NetworkPriority {
    type Resource = LinuxNetwork;

    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");

        if let Some(network) = Self::needs_to_handle(linux_resources) {
            Self::apply(cgroup_root, network)
                .context("failed to apply network priority resource restrictions")?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(network) = &linux_resources.network {
            return Some(network);
        }

        None
    }
}

impl NetworkPriority {
    fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(ni_priorities) = network.priorities.as_ref() {
            let priorities: String = ni_priorities.iter().map(|p| p.to_string()).collect();
            common::write_cgroup_file_str(root_path.join("net_prio.ifpriomap"), priorities.trim())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::LinuxInterfacePriority;

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
            priorities: priorities.into(),
        };

        NetworkPriority::apply(&tmp, &network).expect("apply network priorities");

        let content =
            std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
        assert_eq!(priorities_string.trim(), content);
    }
}
