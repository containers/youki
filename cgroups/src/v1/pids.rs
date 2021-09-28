use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

use super::Controller;
use crate::{
    common,
    stats::{self, PidStats, StatsProvider},
};
use oci_spec::{LinuxPids, LinuxResources};

// Contains the maximum allowed number of active pids
const CGROUP_PIDS_MAX: &str = "pids.max";

pub struct Pids {}

#[async_trait(?Send)]
impl Controller for Pids {
    type Resource = LinuxPids;

    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply pids cgroup config");

        if let Some(pids) = &linux_resources.pids {
            Self::apply(cgroup_root, pids).await?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(pids) = &linux_resources.pids {
            return Some(pids);
        }

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
    async fn apply(root_path: &Path, pids: &LinuxPids) -> Result<()> {
        let limit = if pids.limit > 0 {
            pids.limit.to_string()
        } else {
            "max".to_string()
        };

        common::async_write_cgroup_file_str(&root_path.join(CGROUP_PIDS_MAX), &limit).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{aw, create_temp_dir, set_fixture};
    use oci_spec::LinuxPids;

    // Contains the current number of active pids
    const CGROUP_PIDS_CURRENT: &str = "pids.current";

    #[test]
    fn test_set_pids() {
        let tmp = create_temp_dir("test_set_pids").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_PIDS_MAX, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPids { limit: 1000 };

        aw!(Pids::apply(&tmp, &pids)).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_PIDS_MAX)).expect("Read pids contents");
        assert_eq!(pids.limit.to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let tmp = create_temp_dir("test_set_pids_max").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_PIDS_MAX, "0").expect("set fixture for 0 pids");

        let pids = LinuxPids { limit: 0 };

        aw!(Pids::apply(&tmp, &pids)).expect("apply pids");

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
