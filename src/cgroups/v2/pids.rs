use std::path::Path;

use anyhow::Result;

use crate::cgroups::common;

use super::controller::Controller;
use oci_spec::{LinuxPids, LinuxResources};

pub struct Pids {}

impl Controller for Pids {
    fn apply(linux_resource: &LinuxResources, cgroup_root: &std::path::Path) -> Result<()> {
        log::debug!("Apply pids cgroup v2 config");
        if let Some(pids) = &linux_resource.pids {
            Self::apply(cgroup_root, pids)?;
        }
        Ok(())
    }
}

impl Pids {
    fn apply(root_path: &Path, pids: &LinuxPids) -> Result<()> {
        let limit = if pids.limit > 0 {
            pids.limit.to_string()
        } else {
            "max".to_string()
        };
        common::write_cgroup_file(&root_path.join("pids.max"), &limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::set_fixture;
    use crate::utils::create_temp_dir;
    use oci_spec::LinuxPids;

    #[test]
    fn test_set_pids() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("v2_test_set_pids").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPids { limit: 1000 };

        Pids::apply(&tmp, &pids).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
        assert_eq!(pids.limit.to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("v2_test_set_pids_max").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "0").expect("set fixture for 0 pids");

        let pids = LinuxPids { limit: 0 };

        Pids::apply(&tmp, &pids).expect("apply pids");

        let content =
            std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
        assert_eq!("max".to_string(), content);
    }
}
