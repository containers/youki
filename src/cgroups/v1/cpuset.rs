use std::{fs, path::Path};

use anyhow::Result;
use nix::unistd::Pid;
use oci_spec::{LinuxCpu, LinuxResources};

use crate::cgroups::common::{self, CGROUP_PROCS};

use super::Controller;

const CGROUP_CPUSET_CPUS: &str = "cpuset.cpus";
const CGROUP_CPUSET_MEMS: &str = "cpuset.mems";

pub struct CpuSet {}

impl Controller for CpuSet {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply CpuSet cgroup config");
        fs::create_dir_all(cgroup_root)?;

        if let Some(cpuset) = &linux_resources.cpu {
            Self::apply(cgroup_root, cpuset)?;
        }

        common::write_cgroup_file_(cgroup_root.join(CGROUP_PROCS), pid)?;
        Ok(())
    }
}

impl CpuSet {
    fn apply(root_path: &Path, cpuset: &LinuxCpu) -> Result<()> {
        if let Some(cpus) = &cpuset.cpus {
            common::write_cgroup_file(root_path.join(CGROUP_CPUSET_CPUS), cpus)?;
        }

        if let Some(mems) = &cpuset.mems {
            common::write_cgroup_file(root_path.join(CGROUP_CPUSET_MEMS), mems)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::cgroups::test::{setup, LinuxCpuBuilder};

    #[test]
    fn test_set_cpus() {
        // arrange
        let (tmp, cpus) = setup("test_set_cpus", CGROUP_CPUSET_CPUS);
        let cpuset = LinuxCpuBuilder::new().with_cpus("1-3".to_owned()).build();

        // act
        CpuSet::apply(&tmp, &cpuset).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(&cpus)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPUSET_CPUS));
        assert_eq!(content, "1-3");
    }

    #[test]
    fn test_set_mems() {
        // arrange
        let (tmp, mems) = setup("test_set_mems", CGROUP_CPUSET_MEMS);
        let cpuset = LinuxCpuBuilder::new().with_mems("1-3".to_owned()).build();

        // act
        CpuSet::apply(&tmp, &cpuset).expect("apply cpuset");

        // assert
        let content = fs::read_to_string(&mems)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPUSET_MEMS));
        assert_eq!(content, "1-3");
    }
}
