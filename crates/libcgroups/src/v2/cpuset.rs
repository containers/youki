use anyhow::Result;
use std::path::Path;

use crate::common::{self, ControllerOpt, WrappedIoError};
use oci_spec::runtime::LinuxCpu;

use super::controller::Controller;

const CGROUP_CPUSET_CPUS: &str = "cpuset.cpus";
const CGROUP_CPUSET_MEMS: &str = "cpuset.mems";

pub struct CpuSet {}

impl Controller for CpuSet {
    type Error = WrappedIoError;

    fn apply(controller_opt: &ControllerOpt, cgroup_path: &Path) -> Result<(), Self::Error> {
        if let Some(cpuset) = &controller_opt.resources.cpu() {
            Self::apply(cgroup_path, cpuset)?;
        }

        Ok(())
    }
}

impl CpuSet {
    fn apply(path: &Path, cpuset: &LinuxCpu) -> Result<(), WrappedIoError> {
        if let Some(cpus) = &cpuset.cpus() {
            common::write_cgroup_file_str(path.join(CGROUP_CPUSET_CPUS), cpus)?;
        }

        if let Some(mems) = &cpuset.mems() {
            common::write_cgroup_file_str(path.join(CGROUP_CPUSET_MEMS), mems)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test::setup;
    use oci_spec::runtime::LinuxCpuBuilder;

    #[test]
    fn test_set_cpus() {
        // arrange
        let (tmp, cpus) = setup("test_set_cpus", CGROUP_CPUSET_CPUS);
        let cpuset = LinuxCpuBuilder::default()
            .cpus("1-3".to_owned())
            .build()
            .unwrap();

        // act
        CpuSet::apply(&tmp, &cpuset).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(cpus)
            .unwrap_or_else(|_| panic!("read {CGROUP_CPUSET_CPUS} file content"));
        assert_eq!(content, "1-3");
    }

    #[test]
    fn test_set_mems() {
        // arrange
        let (tmp, mems) = setup("test_set_mems", CGROUP_CPUSET_MEMS);
        let cpuset = LinuxCpuBuilder::default()
            .mems("1-3".to_owned())
            .build()
            .unwrap();

        // act
        CpuSet::apply(&tmp, &cpuset).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(mems)
            .unwrap_or_else(|_| panic!("read {CGROUP_CPUSET_MEMS} file content"));
        assert_eq!(content, "1-3");
    }
}
