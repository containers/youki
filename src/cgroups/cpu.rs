//!
//! Reference: https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cpu

use crate::{
    cgroups::Controller,
    rootfs::default_devices,
    spec::{LinuxCpu, LinuxDeviceType, LinuxResources},
};

use anyhow::Result;
use nix::unistd::Pid;
use std::path::Path;

pub struct Cpu {}

impl Controller for Cpu {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path, pid: Pid) -> Result<()> {
        fs::create_dir_all(cgroup_root)?;

        if let Some(cpu) = &linux_resources.cpu {
            Self::apply(linux_resources, cgroup_root, pid)
        }

        OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(false)
            .open(cgroup_root.join("cgroup.procs"))?
            .write_all(pid.to_string().as_bytes())?;

        Ok(())
    }
}

impl Cpu {
    fn apply_cpu(cpu: &LinuxCpu, cgroup_root: &Path) -> Result<()> {
        if let Some(cpus) = cpu.cpus {
            // validate
            // write to cpuset.cpus
            Self::write_file(cgroup_root.join("cpuset.cpus"), cpus.to_string())?;
        }
        if let Some(mems) = cpu.mems {
            // validate
            // cpuset.mems
            Self::write_file(cgroup_root.join("cpuset.mems"), &mems.to_string())?;
        }
        if let Some(quota) = cpu.quota {
            // cpuset.quota
            Self::write_file(cgroup_root.join("cpuset.quota"), &quota.to_string())?;
        }
        if let Some(period) = cpu.period {
            // cpuset.period
            Self::write_file(cgroup_root.join("cpuset.period"), &period.to_string())?;
        }
        if let Some(realtime_runtime) = cpu.realtime_runtime {
            // cpuset.realtime_runtime
            Self::write_file(
                cgroup_root.join("cpuset.realtime_runtime"),
                &realtime_runtime.to_string(),
            )?;
        }
        if let Some(realtime_period) = cpu.realtime_period {
            // cpuset.realtime_period
            Self::write_file(
                cgroup_root.join("cpuset.realtime_period"),
                &realtime_period.to_string(),
            )?;
        }
        Ok(())
    }

    fn write_file(file_path: &Path, data: &str) -> anyhow::Result<()> {
        fs::OpenOptions::new()
            .create(false)
            .write(true)
            .truncate(true)
            .open(file_path)?
            .write_all(data.as_bytes())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cpu() {}
}
