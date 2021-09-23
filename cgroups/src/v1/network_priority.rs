use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

use super::Controller;
use crate::common;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkPriority {}

#[async_trait(?Send)]
impl Controller for NetworkPriority {
    type Resource = LinuxNetwork;

    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");

        if let Some(network) = Self::needs_to_handle(linux_resources) {
            Self::apply(cgroup_root, network).await?;
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
    async fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(ni_priorities) = network.priorities.as_ref() {
            let priorities: String = ni_priorities.iter().map(|p| p.to_string()).collect();
            common::async_write_cgroup_file_str(
                root_path.join("net_prio.ifpriomap"),
                priorities.trim(),
            )
            .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{aw, create_temp_dir, set_fixture};
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

        aw!(NetworkPriority::apply(&tmp, &network)).expect("apply network priorities");

        let content =
            std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
        assert_eq!(priorities_string.trim(), content);
    }
}
