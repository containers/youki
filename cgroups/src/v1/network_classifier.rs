use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

use super::Controller;
use crate::common;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkClassifier {}

#[async_trait(?Send)]
impl Controller for NetworkClassifier {
    type Resource = LinuxNetwork;

    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply NetworkClassifier cgroup config");

        if let Some(network) = Self::needs_to_handle(linux_resources) {
            Self::apply(cgroup_root, network).await?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(network) = linux_resources.network.as_ref() {
            return Some(network);
        }

        None
    }
}

impl NetworkClassifier {
    async fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(class_id) = network.class_id {
            common::async_write_cgroup_file(root_path.join("net_cls.classid"), class_id).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{aw, create_temp_dir, set_fixture};

    #[test]
    fn test_apply_network_classifier() {
        let tmp = create_temp_dir("test_apply_network_classifier")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_cls.classid", "0").expect("set fixture for classID");

        let id = 0x100001;
        let network = LinuxNetwork {
            class_id: Some(id),
            priorities: Some(vec![]),
        };

        aw!(NetworkClassifier::apply(&tmp, &network)).expect("apply network classID");

        let content =
            std::fs::read_to_string(tmp.join("net_cls.classid")).expect("Read classID contents");
        assert_eq!(id.to_string(), content);
    }
}
