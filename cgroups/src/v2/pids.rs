use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    common::{self, ControllerOpt},
    stats::{self, PidStats, StatsProvider},
};

use super::controller::Controller;
use oci_spec::runtime::LinuxPids;

pub struct Pids {}

impl Controller for Pids {
    fn apply(controller_opt: &ControllerOpt, cgroup_root: &std::path::Path) -> Result<()> {
        log::debug!("Apply pids cgroup v2 config");
        if let Some(pids) = &controller_opt.resources.pids() {
            Self::apply(cgroup_root, pids).context("failed to apply pids resource restrictions")?;
        }
        Ok(())
    }
}

impl StatsProvider for Pids {
    type Stats = PidStats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        stats::pid_stats(cgroup_path)
    }
}

impl Pids {
    fn apply(root_path: &Path, pids: &LinuxPids) -> Result<()> {
        let limit = if pids.limit() > 0 {
            pids.limit().to_string()
        } else {
            "max".to_string()
        };
        common::write_cgroup_file(&root_path.join("pids.max"), &limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::LinuxPidsBuilder;

    #[test]
    fn test_set_pids() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("v2_test_set_pids").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPidsBuilder::default().limit(1000).build().unwrap();

        Pids::apply(&tmp, &pids).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
        assert_eq!(pids.limit().to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let pids_file_name = "pids.max";
        let tmp = create_temp_dir("v2_test_set_pids_max").expect("create temp directory for test");
        set_fixture(&tmp, pids_file_name, "0").expect("set fixture for 0 pids");

        let pids = LinuxPidsBuilder::default().limit(0).build().unwrap();

        Pids::apply(&tmp, &pids).expect("apply pids");

        let content =
            std::fs::read_to_string(tmp.join(pids_file_name)).expect("Read pids contents");
        assert_eq!("max".to_string(), content);
    }
}
