use std::path::Path;

use crate::{
    common::{self, ControllerOpt, WrappedIoError},
    stats::{self, PidStats, PidStatsError, StatsProvider},
};

use super::controller::Controller;
use oci_spec::runtime::LinuxPids;

pub struct Pids {}

impl Controller for Pids {
    type Error = WrappedIoError;

    fn apply(
        controller_opt: &ControllerOpt,
        cgroup_root: &std::path::Path,
    ) -> Result<(), Self::Error> {
        tracing::debug!("Apply pids cgroup v2 config");
        if let Some(pids) = &controller_opt.resources.pids() {
            Self::apply(cgroup_root, pids)?;
        }
        Ok(())
    }
}

impl StatsProvider for Pids {
    type Error = PidStatsError;
    type Stats = PidStats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats, Self::Error> {
        stats::pid_stats(cgroup_path)
    }
}

impl Pids {
    fn apply(root_path: &Path, pids: &LinuxPids) -> Result<(), WrappedIoError> {
        let limit = if pids.limit() > 0 {
            pids.limit().to_string()
        } else {
            "max".to_string()
        };
        common::write_cgroup_file(root_path.join("pids.max"), limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::set_fixture;
    use oci_spec::runtime::LinuxPidsBuilder;

    #[test]
    fn test_set_pids() {
        let pids_file_name = "pids.max";
        let tmp = tempfile::tempdir().unwrap();
        set_fixture(tmp.path(), pids_file_name, "1000").expect("Set fixture for 1000 pids");

        let pids = LinuxPidsBuilder::default().limit(1000).build().unwrap();

        Pids::apply(tmp.path(), &pids).expect("apply pids");
        let content =
            std::fs::read_to_string(tmp.path().join(pids_file_name)).expect("Read pids contents");
        assert_eq!(pids.limit().to_string(), content);
    }

    #[test]
    fn test_set_pids_max() {
        let pids_file_name = "pids.max";
        let tmp = tempfile::tempdir().unwrap();
        set_fixture(tmp.path(), pids_file_name, "0").expect("set fixture for 0 pids");

        let pids = LinuxPidsBuilder::default().limit(0).build().unwrap();

        Pids::apply(tmp.path(), &pids).expect("apply pids");

        let content =
            std::fs::read_to_string(tmp.path().join(pids_file_name)).expect("Read pids contents");
        assert_eq!("max".to_string(), content);
    }
}
