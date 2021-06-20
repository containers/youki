use std::{fs::create_dir_all, path::Path};

use anyhow::Result;
use nix::unistd::Pid;

use crate::cgroups::common;
use crate::cgroups::common::CGROUP_PROCS;
use crate::cgroups::v1::Controller;
use oci_spec::{LinuxNetwork, LinuxResources};

pub struct NetworkClassifier {}

impl Controller for NetworkClassifier {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply NetworkClassifier cgroup config");
        create_dir_all(&cgroup_root)?;

        if let Some(network) = linux_resources.network.as_ref() {
            Self::apply(cgroup_root, network)?;
        }

        common::write_cgroup_file(cgroup_root.join(CGROUP_PROCS), pid)?;
        Ok(())
    }
}

impl NetworkClassifier {
    fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(class_id) = network.class_id {
            common::write_cgroup_file(root_path.join("net_cls.classid"), class_id)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::cgroups::test::set_fixture;
    use crate::utils::create_temp_dir;
    use super::*;

    #[test]
    fn test_apply_network_classifier() {
        let tmp = create_temp_dir("test_apply_network_classifier")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_cls.classid", "0").expect("set fixture for classID");

        let id = 0x100001;
        let network = LinuxNetwork {
            class_id: Some(id),
            priorities: vec![],
        };

        NetworkClassifier::apply(&tmp, &network).expect("apply network classID");

        let content =
            std::fs::read_to_string(tmp.join("net_cls.classid")).expect("Read classID contents");
        assert_eq!(id.to_string(), content);
    }
}
