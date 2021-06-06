use nix::unistd::Pid;
use crate::cgroups::common::{self};
use oci_spec::{LinuxCpu, LinuxResources};
use std::{fs, path::Path};
use anyhow::Result;

use super::{Controller};

pub struct CpuAcct {}

impl Controller for CpuAcct {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply CpuAcct cgroup config");
        fs::create_dir_all(&cgroup_root)?;
        
        if let Some(cpuset) = &linux_resources.cpu {
            Self::apply(cgroup_root, cpuset, pid)?;
        }

        Ok(())
    }
}

impl CpuAcct {
    fn apply(cgroup_root: &Path, cpuset: &LinuxCpu, pid: Pid) -> Result<()> {
        if let Some(cpus) = &cpuset.cpus {
            common::write_cgroup_file_str(cgroup_root.join(pid.to_string()), cpus)?;
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
    fn test_acct_cpu() {
    }
}
