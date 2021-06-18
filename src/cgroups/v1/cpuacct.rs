use std::{fs, path::Path};

use anyhow::Result;
use nix::unistd::Pid;
use oci_spec::LinuxResources;

use crate::cgroups::common::{self, CGROUP_PROCS};

use super::Controller;

pub struct CpuAcct {}

impl Controller for CpuAcct {
    fn apply(_linux_resources: &LinuxResources, cgroup_path: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply cpuacct cgroup config");
        fs::create_dir_all(cgroup_path)?;

        common::write_cgroup_file(cgroup_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::setup;

    #[test]
    fn test_apply() {
        let (tmp, procs) = setup("test_cpuacct_apply", CGROUP_PROCS);
        let resource = LinuxResources::default();
        let pid = Pid::from_raw(1000);

        CpuAcct::apply(&resource, &tmp, pid).expect("apply cpuacct");

        let content = fs::read_to_string(&procs)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_PROCS));
        assert_eq!(content, "1000");
    }
}
