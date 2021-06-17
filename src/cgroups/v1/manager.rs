use std::fs;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use nix::unistd::Pid;

use procfs::process::Process;

use super::{
    blkio::Blkio, controller_type::CONTROLLERS, cpu::Cpu, cpuacct::CpuAcct, cpuset::CpuSet,
    devices::Devices, freezer::Freezer, hugetlb::Hugetlb, memory::Memory,
    network_classifier::NetworkClassifier, network_priority::NetworkPriority, pids::Pids, util,
    Controller,
};

use crate::cgroups::common::CGROUP_PROCS;
use crate::utils;
use crate::{cgroups::common::CgroupManager, utils::PathBufExt};
use oci_spec::LinuxResources;
pub struct Manager {
    subsystems: HashMap<String, PathBuf>,
}

impl Manager {
    pub fn new(cgroup_path: PathBuf) -> Result<Self> {
        let mut subsystems = HashMap::<String, PathBuf>::new();
        for subsystem in CONTROLLERS.iter().map(|c| c.to_string()) {
            subsystems.insert(
                subsystem.to_owned(),
                Self::get_subsystem_path(&cgroup_path, &subsystem)?,
            );
        }

        Ok(Manager { subsystems })
    }

    fn get_subsystem_path(cgroup_path: &Path, subsystem: &str) -> anyhow::Result<PathBuf> {
        log::debug!("Get path for subsystem: {}", subsystem);
        let mount_point = util::get_subsystem_mount_points(subsystem)?;

        let cgroup = Process::myself()?
            .cgroups()?
            .into_iter()
            .find(|c| c.controllers.contains(&subsystem.to_owned()))
            .unwrap();

        let p = if cgroup_path.to_string_lossy().into_owned().is_empty() {
            mount_point.join_absolute_path(Path::new(&cgroup.pathname))?
        } else if cgroup_path.is_absolute() {
            mount_point.join_absolute_path(&cgroup_path)?
        } else {
            mount_point.join(cgroup_path)
        };

        Ok(p)
    }
}

impl CgroupManager for Manager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()> {
        for subsys in &self.subsystems {
            match subsys.0.as_str() {
                "cpu" => Cpu::apply(linux_resources, &subsys.1, pid)?,
                "cpuacct" => CpuAcct::apply(linux_resources, &subsys.1, pid)?,
                "cpuset" => CpuSet::apply(linux_resources, &subsys.1, pid)?,
                "devices" => Devices::apply(linux_resources, &subsys.1, pid)?,
                "hugetlb" => Hugetlb::apply(linux_resources, &subsys.1, pid)?,
                "memory" => Memory::apply(linux_resources, &subsys.1, pid)?,
                "pids" => Pids::apply(linux_resources, &subsys.1, pid)?,
                "blkio" => Blkio::apply(linux_resources, &subsys.1, pid)?,
                "net_prio" => NetworkPriority::apply(linux_resources, &subsys.1, pid)?,
                "net_cls" => NetworkClassifier::apply(linux_resources, &subsys.1, pid)?,
                "freezer" => Freezer::apply(linux_resources, &subsys.1, pid)?,
                _ => unreachable!("every subsystem should have an associated controller"),
            }
        }

        Ok(())
    }

    fn remove(&self) -> Result<()> {
        for cgroup_path in &self.subsystems {
            if cgroup_path.1.exists() {
                log::debug!("remove cgroup {:?}", cgroup_path.1);
                let procs_path = cgroup_path.1.join(CGROUP_PROCS);
                let procs = fs::read_to_string(&procs_path)?;

                for line in procs.lines() {
                    let pid: i32 = line.parse()?;
                    let _ = nix::sys::signal::kill(Pid::from_raw(pid), nix::sys::signal::SIGKILL);
                }

                utils::delete_with_retry(cgroup_path.1)?;
            }
        }

        Ok(())
    }
}
