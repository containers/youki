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
