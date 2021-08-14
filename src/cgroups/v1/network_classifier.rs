use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use rio::Rio;

use crate::cgroups::common;
use crate::cgroups::v1::Controller;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkClassifier {}

#[async_trait]
impl Controller for NetworkClassifier {
    type Resource = LinuxNetwork;

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply NetworkClassifier cgroup config");

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

impl NetworkClassifier {
    async fn apply(ring: &Rio, root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(class_id) = network.class_id {
            let file = common::open_cgroup_file(root_path.join("net_cls.classid"))?;
            common::async_write_cgroup_file(ring, &file, class_id).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::{set_fixture, aw};
    use crate::utils::create_temp_dir;

    #[test]
    fn test_apply_network_classifier() {
        let tmp = create_temp_dir("test_apply_network_classifier")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_cls.classid", "0").expect("set fixture for classID");
        let ring = rio::new().expect("start io_uring");

        let id = 0x100001;
        let network = LinuxNetwork {
            class_id: Some(id),
            priorities: vec![],
        };

        aw!(NetworkClassifier::apply(&ring, &tmp, &network)).expect("apply network classID");

        let content =
            std::fs::read_to_string(tmp.join("net_cls.classid")).expect("Read classID contents");
        assert_eq!(id.to_string(), content);
    }
}
