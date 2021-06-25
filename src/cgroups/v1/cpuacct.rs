use std::path::Path;

use anyhow::Result;
use oci_spec::LinuxResources;

use super::Controller;

pub struct CpuAcct {}

impl Controller for CpuAcct {
    fn apply(_linux_resources: &LinuxResources, _cgroup_path: &Path) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use nix::unistd::Pid;

    use super::*;
    use crate::cgroups::{common::CGROUP_PROCS, test::setup};

    #[test]
    fn test_add_task() {
        let (tmp, procs) = setup("test_cpuacct_apply", CGROUP_PROCS);
        let pid = Pid::from_raw(1000);

        CpuAcct::add_task(pid, &tmp).expect("apply cpuacct");

        let content = fs::read_to_string(&procs)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_PROCS));
        assert_eq!(content, "1000");
    }
}
