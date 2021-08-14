use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use rio::Rio;

use crate::cgroups::common;
use crate::cgroups::v1::Controller;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkPriority {}

#[async_trait]
impl Controller for NetworkPriority {
    type Resource = LinuxNetwork;

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");

        if let Some(network) = Self::needs_to_handle(linux_resources) {
            Self::apply(ring, cgroup_root, network).await?;
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
    async fn apply(ring: &Rio, root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        let priorities: String = network.priorities.iter().map(|p| p.to_string()).collect();
        let file = common::open_cgroup_file(root_path.join("net_prio.ifpriomap"))?;
        common::async_write_cgroup_file_str(ring, &file, &priorities.trim()).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::{set_fixture, aw};
    use crate::utils::create_temp_dir;
    use oci_spec::LinuxInterfacePriority;

    #[test]
    fn test_apply_network_priorites() {
        let tmp = create_temp_dir("test_apply_network_priorites")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_prio.ifpriomap", "").expect("set fixture for priority map");
        let ring = rio::new().expect("start io_uring");

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

        aw!(NetworkPriority::apply(&ring, &tmp, &network)).expect("apply network priorities");

        let content =
            std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
        assert_eq!(priorities_string.trim(), content);
    }
}
