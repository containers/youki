use std::{path::Path};
use anyhow::{Result};

use oci_spec::{LinuxCpu, LinuxResources};
use crate::{cgroups::common};

use super::{controller::Controller };

const CGROUP_CPUSET_CPUS: &str = "cpuset.cpus";
const CGROUP_CPUSET_MEMS: &str = "cpuset.mems";

pub struct CpuSet {}

impl Controller for CpuSet {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        if let Some(cpuset) = &linux_resources.cpu {
            Self::apply(cgroup_path, cpuset)?;
        }

        Ok(())
    }
}

impl CpuSet {
    fn apply(path: &Path, cpuset: &LinuxCpu) -> Result<()> {
        if let Some(cpus) = &cpuset.cpus {
            common::write_cgroup_file(&path.join(CGROUP_CPUSET_CPUS), cpus)?;
        }

        if let Some(mems) = &cpuset.mems {
            common::write_cgroup_file(&path.join(CGROUP_CPUSET_MEMS), mems)?;
        }

        Ok(())
    }
}

