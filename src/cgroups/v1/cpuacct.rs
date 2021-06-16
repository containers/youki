use std::{fs, path::Path};

use anyhow::Result;
use nix::unistd::Pid;
use oci_spec::LinuxResources;

use crate::cgroups::common::{self, CGROUP_PROCS};

use super::Controller;

const CGROUP_CPUACCT_TASKS: &str = "tasks";

pub struct CpuAcct {}

impl Controller for CpuAcct {
    fn apply(_linux_resources: &LinuxResources, cgroup_path: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply cpuacct cgroup config");
        fs::create_dir_all(cgroup_path)?;

        Self::apply(cgroup_path, pid)?;

        common::write_cgroup_file(cgroup_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }
}

impl CpuAcct {
    fn apply(root_path: &Path, pid: Pid) -> Result<()> {
        common::write_cgroup_file_str(root_path.join(CGROUP_CPUACCT_TASKS), &pid.to_string())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::cgroups::test::setup;
    use nix::unistd::Pid;

    #[test]
    fn test_set_cpuacct() {
        // arrange
        let (tmp, cpuacct) = setup("test_set_cpuacct", CGROUP_CPUACCT_TASKS);
        let pid = Pid::from_raw(1000);

        // act
        CpuAcct::apply(&tmp, pid).expect("apply cpuacct");

        // assert
        let content = fs::read_to_string(&cpuacct)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPUACCT_TASKS));
        assert_eq!(content, "1000");
    }
}
