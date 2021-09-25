use std::path::Path;

use anyhow::{Context, Result};

use super::Controller;
use crate::{
    common::{self, ControllerOpt},
    stats::{self, PidStats, StatsProvider},
};
use oci_spec::runtime::LinuxPids;

// Contains the maximum allowed number of active pids
const CGROUP_PIDS_MAX: &str = "pids.max";

pub struct Pids {}

impl Controller for Pids {
    type Resource = LinuxPids;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply pids cgroup config");

        if let Some(pids) = &controller_opt.resources.pids() {
            Self::apply(cgroup_root, pids).context("failed to apply pids resource restrictions")?;
        }

        Ok(())
    }

    fn needs_to_handle<'a>(_controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        // TODO: fix compile error
        // error[E0515]: cannot return value referencing temporary value
        // if let Some(pids) = &controller_opt.resources.pids() {
        //     return Some(pids);
        // }

        None
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

        common::write_cgroup_file_str(&root_path.join(CGROUP_PIDS_MAX), &limit)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::LinuxPidsBuilder;

    // Contains the current number of active pids
    const CGROUP_PIDS_CURRENT: &str = "pids.current";

    #[test]
    fn test_set_pids() {
        let tmp = create_temp_dir("test_set_pids").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_PIDS_MAX, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPidsBuilder::default().limit(1000).build().unwrap();

        Pids::apply(&tmp, &pids).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_PIDS_MAX)).expect("Read pids contents");
        assert_eq!(pids.limit().to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let tmp = create_temp_dir("test_set_pids_max").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_PIDS_MAX, "0").expect("set fixture for 0 pids");

        let pids = LinuxPidsBuilder::default().limit(0).build().unwrap();

        Pids::apply(&tmp, &pids).expect("apply pids");

        let content =
            std::fs::read_to_string(tmp.join(CGROUP_PIDS_MAX)).expect("Read pids contents");
        assert_eq!("max".to_string(), content);
    }

    #[test]
    fn test_stat_pids() {
        let tmp = create_temp_dir("test_stat_pids").expect("create temp dir for test");
        set_fixture(&tmp, CGROUP_PIDS_CURRENT, "5\n").unwrap();
        set_fixture(&tmp, CGROUP_PIDS_MAX, "30\n").unwrap();

        let stats = Pids::stats(&tmp).expect("get cgroup stats");

        assert_eq!(stats.current, 5);
        assert_eq!(stats.limit, 30);
    }

    #[test]
    fn test_stat_pids_max() {
        let tmp = create_temp_dir("test_stat_pids_max").expect("create temp dir for test");
        set_fixture(&tmp, CGROUP_PIDS_CURRENT, "5\n").unwrap();
        set_fixture(&tmp, CGROUP_PIDS_MAX, "max\n").unwrap();

        let stats = Pids::stats(&tmp).expect("get cgroup stats");

        assert_eq!(stats.current, 5);
        assert_eq!(stats.limit, 0);
    }
}
