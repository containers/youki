use std::path::Path;

use anyhow::{Context, Result};

use crate::cgroups::{
    common,
    stats::{PidStats, StatsProvider},
    v1::Controller,
};
use oci_spec::{LinuxPids, LinuxResources};

// Contains the current number of active pids
const CGROUP_PIDS_CURRENT: &str = "pids.current";
// Contains the maximum allowed number of active pids
const CGROUP_PIDS_MAX: &str = "pids.max";

pub struct Pids {}

impl Controller for Pids {
    type Resource = LinuxPids;

    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply pids cgroup config");

        if let Some(pids) = &linux_resources.pids {
            Self::apply(cgroup_root, pids)?;
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
        let mut stats = PidStats::default();

        let current = common::read_cgroup_file(cgroup_path.join(CGROUP_PIDS_CURRENT))?;
        stats.current = current
            .trim()
            .parse()
            .context("failed to parse current pids")?;

        let limit = common::read_cgroup_file(cgroup_path.join(CGROUP_PIDS_MAX))
            .map(|l| l.trim().to_owned())?;
        if limit != "max" {
            stats.limit = limit.parse().context("failed to parse pids limit")?;
        }

        Ok(stats)
    }
}

impl Pids {
    fn apply(root_path: &Path, pids: &LinuxPids) -> Result<()> {
        let limit = if pids.limit > 0 {
            pids.limit.to_string()
        } else {
            "max".to_string()
        };

        common::write_cgroup_file_str(&root_path.join("pids.max"), &limit)?;
        Ok(())
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
        let tmp = create_temp_dir("test_set_pids").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_PIDS_MAX, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPids { limit: 1000 };

        Pids::apply(&tmp, &pids).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.join(CGROUP_PIDS_MAX)).expect("Read pids contents");
        assert_eq!(pids.limit.to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let tmp = create_temp_dir("test_set_pids_max").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_PIDS_MAX, "0").expect("set fixture for 0 pids");

        let pids = LinuxPids { limit: 0 };

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
