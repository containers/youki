use std::fs;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

use anyhow::bail;
use anyhow::Result;
use nix::unistd::Pid;

use procfs::process::Process;

use super::ControllerType as CtrlType;
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
    subsystems: HashMap<CtrlType, PathBuf>,
}

impl Manager {
    /// Constructs a new cgroup manager with cgroups_path being relative to the root of the subsystem
    pub fn new(cgroup_path: PathBuf) -> Result<Self> {
        let mut subsystems = HashMap::<CtrlType, PathBuf>::new();
        for subsystem in CONTROLLERS {
            if let Ok(subsystem_path) = Self::get_subsystem_path(&cgroup_path, subsystem) {
                subsystems.insert(subsystem.clone(), subsystem_path);
            } else {
                log::warn!("Cgroup {} not supported on this system", subsystem);
            }
        }

        Ok(Manager { subsystems })
    }

    fn get_subsystem_path(cgroup_path: &Path, subsystem: &CtrlType) -> Result<PathBuf> {
        log::debug!("Get path for subsystem: {}", subsystem);
        let mount_point = util::get_subsystem_mount_point(subsystem)?;

        let cgroup = Process::myself()?
            .cgroups()?
            .into_iter()
            .find(|c| c.controllers.contains(&subsystem.to_string()))
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

    fn get_required_controllers(
        &self,
        linux_resources: &LinuxResources,
    ) -> Result<HashMap<&CtrlType, &PathBuf>> {
        let mut required_controllers = HashMap::new();

        for controller in CONTROLLERS {
            let required = match controller {
                CtrlType::Cpu => Cpu::needs_to_handle(linux_resources).is_some(),
                CtrlType::CpuAcct => CpuAcct::needs_to_handle(linux_resources).is_some(),
                CtrlType::CpuSet => CpuSet::needs_to_handle(linux_resources).is_some(),
                CtrlType::Devices => Devices::needs_to_handle(linux_resources).is_some(),
                CtrlType::HugeTlb => Hugetlb::needs_to_handle(linux_resources).is_some(),
                CtrlType::Memory => Memory::needs_to_handle(linux_resources).is_some(),
                CtrlType::Pids => Pids::needs_to_handle(linux_resources).is_some(),
                CtrlType::Blkio => Blkio::needs_to_handle(linux_resources).is_some(),
                CtrlType::NetworkPriority => {
                    NetworkPriority::needs_to_handle(linux_resources).is_some()
                }
                CtrlType::NetworkClassifier => {
                    NetworkClassifier::needs_to_handle(linux_resources).is_some()
                }
                CtrlType::Freezer => Freezer::needs_to_handle(linux_resources).is_some(),
            };

            if required {
                if let Some(subsystem_path) = self.subsystems.get(controller) {
                    required_controllers.insert(controller, subsystem_path);
                } else {
                    bail!("Cgroup {} is required to fullfill the request, but is not supported by this system", controller);
                }
            }
        }

        Ok(required_controllers)
    }
}

impl CgroupManager for Manager {
    fn add_task(&self, pid: Pid) -> Result<()> {
        for subsys in &self.subsystems {
            match subsys.0 {
                CtrlType::Cpu => Cpu::add_task(pid, subsys.1)?,
                CtrlType::CpuAcct => CpuAcct::add_task(pid, subsys.1)?,
                CtrlType::CpuSet => CpuSet::add_task(pid, subsys.1)?,
                CtrlType::Devices => Devices::add_task(pid, subsys.1)?,
                CtrlType::HugeTlb => Hugetlb::add_task(pid, subsys.1)?,
                CtrlType::Memory => Memory::add_task(pid, subsys.1)?,
                CtrlType::Pids => Pids::add_task(pid, subsys.1)?,
                CtrlType::Blkio => Blkio::add_task(pid, subsys.1)?,
                CtrlType::NetworkPriority => NetworkPriority::add_task(pid, subsys.1)?,
                CtrlType::NetworkClassifier => NetworkClassifier::add_task(pid, subsys.1)?,
                _ => continue,
            }
        }

        Ok(())
    }

    fn apply(&self, linux_resources: &LinuxResources) -> Result<()> {
        for subsys in self.get_required_controllers(linux_resources)? {
            match subsys.0 {
                CtrlType::Cpu => Cpu::apply(linux_resources, &subsys.1)?,
                CtrlType::CpuAcct => CpuAcct::apply(linux_resources, &subsys.1)?,
                CtrlType::CpuSet => CpuSet::apply(linux_resources, &subsys.1)?,
                CtrlType::Devices => Devices::apply(linux_resources, &subsys.1)?,
                CtrlType::HugeTlb => Hugetlb::apply(linux_resources, &subsys.1)?,
                CtrlType::Memory => Memory::apply(linux_resources, &subsys.1)?,
                CtrlType::Pids => Pids::apply(linux_resources, &subsys.1)?,
                CtrlType::Blkio => Blkio::apply(linux_resources, &subsys.1)?,
                CtrlType::NetworkPriority => NetworkPriority::apply(linux_resources, &subsys.1)?,
                CtrlType::NetworkClassifier => {
                    NetworkClassifier::apply(linux_resources, &subsys.1)?
                }
                CtrlType::Freezer => Freezer::apply(linux_resources, &subsys.1)?,
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
