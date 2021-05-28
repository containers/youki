//!
//! Reference: https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cpu

use crate::{
    cgroups::Controller,
    spec::{LinuxCpu, LinuxResources},
};

use anyhow::Result;
use nix::unistd::Pid;
use std::{io::Write, path::Path};

pub struct Cpu {}

impl Controller for Cpu {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        std::fs::create_dir_all(cgroup_root)?;

        if let Some(cpu) = &linux_resources.cpu {
            // We should set the real-Time group scheduling settings before moving
            // in the process because if the process is already in SCHED_RR mode
            // and no RT bandwidth is set, adding it will fail.
            // https://github.com/opencontainers/runc/blob/3f6594675675d4e88901c782462f56497260b1d2/libcontainer/cgroups/fs/cpu.go
            Self::apply_scheduler(cpu, &cgroup_root)?;
            Self::apply_cpu(cpu, &cgroup_root)?;
        }

        std::fs::OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(&cgroup_root.join("cgroup.procs"))?
            .write_all(pid.to_string().as_bytes())?;

        Ok(())
    }
}

impl Cpu {
    fn apply_cpu(cpu: &LinuxCpu, cgroup_root: &Path) -> Result<()> {
        Self::set(&cgroup_root.join("cpuset.cpus"), &cpu.cpus)?;
        Self::set(&cgroup_root.join("cpuset.mems"), &cpu.mems)?;
        if let Some(shares) = &cpu.shares {
            Self::set(&cgroup_root.join("cpu.shares"), &shares)?;
        };
        if let Some(quota) = &cpu.quota {
            Self::set(&cgroup_root.join("cpu.quota"), &quota)?;
        };
        if let Some(period) = &cpu.period {
            Self::set(&cgroup_root.join("cpu.period"), &period)?;
        };
        Ok(())
    }

    fn apply_scheduler(cpu: &LinuxCpu, cgroup_root: &Path) -> Result<()> {
        if let Some(realtime_runtime) = cpu.realtime_runtime {
            Self::set(&cgroup_root.join("cpu.rt_runtime_us"), &realtime_runtime)?;
        }
        if let Some(realtime_period) = cpu.realtime_period {
            Self::set(&cgroup_root.join("cpu.rt_period_us"), &realtime_period)?;
        }
        Ok(())
    }

    fn set<S: ToString>(file_path: &Path, data: S) -> anyhow::Result<()> {
        Ok(std::fs::OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(file_path)?
            .write_all(data.to_string().as_bytes())?)
    }
}

#[cfg(test)]
mod tests {
    use std::{env::temp_dir, path::PathBuf};

    use super::*;

    fn create_temp_dir(test_name: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(temp_dir().join(test_name))?;
        Ok(temp_dir().join(test_name))
    }

    #[test]
    fn test_cpu_apply() {}
}
