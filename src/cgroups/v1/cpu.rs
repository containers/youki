use std::{fs, path::Path};

use anyhow::Result;
use nix::unistd::Pid;
use oci_spec::{LinuxCpu, LinuxResources};

use crate::cgroups::common::{self, CGROUP_PROCS};

use super::Controller;

const CGROUP_CPU_SHARES: &str = "cpu.shares";
const CGROUP_CPU_QUOTA: &str = "cpu.cfs_quota_us";
const CGROUP_CPU_PERIOD: &str = "cpu.cfs_period_us";
const CGROUP_CPU_RT_RUNTIME: &str = "cpu.rt_runtime_us";
const CGROUP_CPU_RT_PERIOD: &str = "cpu.rt_period_us";

pub struct Cpu {}

impl Controller for Cpu {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        log::debug!("Apply Cpu cgroup config");
        fs::create_dir_all(cgroup_root)?;
        if let Some(cpu) = &linux_resources.cpu {
            Self::apply(cgroup_root, cpu)?;
        }

        common::write_cgroup_file_(cgroup_root.join(CGROUP_PROCS), pid)?;
        Ok(())
    }
}

impl Cpu {
    fn apply(root_path: &Path, cpu: &LinuxCpu) -> Result<()> {
        if let Some(cpu_shares) = cpu.shares {
            if cpu_shares != 0 {
                common::write_cgroup_file_(root_path.join(CGROUP_CPU_SHARES), cpu_shares)?;
            }
        }

        if let Some(cpu_period) = cpu.period {
            if cpu_period != 0 {
                common::write_cgroup_file_(root_path.join(CGROUP_CPU_PERIOD), cpu_period)?;
            }
        }

        if let Some(cpu_quota) = cpu.quota {
            if cpu_quota != 0 {
                common::write_cgroup_file_(root_path.join(CGROUP_CPU_QUOTA), cpu_quota)?;
            }
        }

        if let Some(rt_runtime) = cpu.realtime_runtime {
            if rt_runtime != 0 {
                common::write_cgroup_file_(root_path.join(CGROUP_CPU_RT_RUNTIME), rt_runtime)?;
            }
        }

        if let Some(rt_period) = cpu.realtime_period {
            if rt_period != 0 {
                common::write_cgroup_file_(root_path.join(CGROUP_CPU_RT_PERIOD), rt_period)?;
            }
        }

        Ok(())
    }
}
