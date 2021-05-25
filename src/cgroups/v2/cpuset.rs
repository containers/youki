use std::{path::Path};
use anyhow::Result;

use oci_spec::{LinuxCpu, LinuxResources};

use super::{controller::Controller, common };

const CGROUP_CPUSET_CPUS: &str = "cpuset.cpus";
const CGROUP_CPUSET_MEMS: &str = "cpuset.mems";

pub struct CpuSet {}

impl Controller for CpuSet {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        match &linux_resources.cpu {
            None => return Ok(()),
            Some(cpu) => Self::apply(cgroup_path, &cpu)?,
        }

        Ok(())
    }
}

impl CpuSet {
    fn apply(path: &Path, cpuset: &LinuxCpu) -> Result<()> {
        if cpuset.cpus.is_some() {
            common::write_cgroup_file(&path.join(CGROUP_CPUSET_CPUS), cpuset.cpus.as_ref().unwrap())?;
        }

        if cpuset.mems.is_some() {
            common::write_cgroup_file(&path.join(CGROUP_CPUSET_MEMS), cpuset.mems.as_ref().unwrap())?;
        }

        Ok(())
    }
}

