use std::{
    fs::{self},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};

use nix::unistd::Pid;
use oci_spec::{FreezerState, LinuxResources};

#[cfg(feature = "cgroupsv2_devices")]
use super::devices::Devices;
use super::{
    controller::Controller,
    controller_type::{
        ControllerType, PseudoControllerType, CONTROLLER_TYPES, PSEUDO_CONTROLLER_TYPES,
    },
    cpu::Cpu,
    cpuset::CpuSet,
    freezer::Freezer,
    hugetlb::HugeTlb,
    io::Io,
    memory::Memory,
    pids::Pids,
    unified::Unified,
};
use crate::{
    common::{self, CgroupManager, PathBufExt, CGROUP_PROCS},
    stats::{Stats, StatsProvider},
};

const CGROUP_CONTROLLERS: &str = "cgroup.controllers";
const CGROUP_SUBTREE_CONTROL: &str = "cgroup.subtree_control";

pub struct Manager {
    root_path: PathBuf,
    cgroup_path: PathBuf,
    full_path: PathBuf,
}

impl Manager {
    /// Constructs a new cgroup manager with root path being the mount point
    /// of a cgroup v2 fs and cgroup path being a relative path from the root
    pub fn new(root_path: PathBuf, cgroup_path: PathBuf) -> Result<Self> {
        let full_path = root_path.join_safely(&cgroup_path)?;

        Ok(Self {
            root_path,
            cgroup_path,
            full_path,
        })
    }

    fn create_unified_cgroup(&self, pid: Pid) -> Result<()> {
        let controllers: Vec<String> = self
            .get_available_controllers()?
            .iter()
            .map(|c| format!("{}{}", "+", c.to_string()))
            .collect();

        Self::write_controllers(&self.root_path, &controllers)?;

        let mut current_path = self.root_path.clone();
        let mut components = self.cgroup_path.components().skip(1).peekable();
        while let Some(component) = components.next() {
            current_path = current_path.join(component);
            if !current_path.exists() {
                fs::create_dir(&current_path)?;
                fs::metadata(&current_path)?.permissions().set_mode(0o755);
            }

            // last component cannot have subtree_control enabled due to internal process constraint
            // if this were set, writing to the cgroups.procs file will fail with Erno 16 (device or resource busy)
            if components.peek().is_some() {
                Self::write_controllers(&current_path, &controllers)?;
            }
        }

        common::write_cgroup_file(&self.full_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }

    fn get_available_controllers(&self) -> Result<Vec<ControllerType>> {
        let controllers_path = self.root_path.join(CGROUP_CONTROLLERS);
        if !controllers_path.exists() {
            bail!(
                "cannot get available controllers. {:?} does not exist",
                controllers_path
            )
        }

        let mut controllers = Vec::new();
        for controller in fs::read_to_string(&controllers_path)?.split_whitespace() {
            match controller {
                "cpu" => controllers.push(ControllerType::Cpu),
                "cpuset" => controllers.push(ControllerType::CpuSet),
                "hugetlb" => controllers.push(ControllerType::HugeTlb),
                "io" => controllers.push(ControllerType::Io),
                "memory" => controllers.push(ControllerType::Memory),
                "pids" => controllers.push(ControllerType::Pids),
                "freezer" => controllers.push(ControllerType::Freezer),
                tpe => log::warn!("Controller {} is not yet implemented.", tpe),
            }
        }

        Ok(controllers)
    }

    fn write_controllers(path: &Path, controllers: &[String]) -> Result<()> {
        for controller in controllers {
            common::write_cgroup_file_str(path.join(CGROUP_SUBTREE_CONTROL), controller)?;
        }

        Ok(())
    }
}

impl CgroupManager for Manager {
    fn add_task(&self, pid: Pid) -> Result<()> {
        self.create_unified_cgroup(pid)?;
        Ok(())
    }

    fn apply(&self, linux_resources: &LinuxResources) -> Result<()> {
        for controller in CONTROLLER_TYPES {
            match controller {
                ControllerType::Cpu => Cpu::apply(linux_resources, &self.full_path)?,
                ControllerType::CpuSet => CpuSet::apply(linux_resources, &self.full_path)?,
                ControllerType::HugeTlb => HugeTlb::apply(linux_resources, &self.full_path)?,
                ControllerType::Io => Io::apply(linux_resources, &self.full_path)?,
                ControllerType::Memory => Memory::apply(linux_resources, &self.full_path)?,
                ControllerType::Pids => Pids::apply(linux_resources, &self.full_path)?,
                ControllerType::Freezer => Freezer::apply(linux_resources, &self.full_path)?,
            }
        }

        #[cfg(feature = "cgroupsv2_devices")]
        Devices::apply(linux_resources, &self.cgroup_path)?;

        for pseudoctlr in PSEUDO_CONTROLLER_TYPES {
            match pseudoctlr {
                PseudoControllerType::Unified => Unified::apply(
                    linux_resources,
                    &self.cgroup_path,
                    self.get_available_controllers()?,
                )?,
                _ => {}
            }
        }

        Ok(())
    }

    fn remove(&self) -> Result<()> {
        log::debug!("remove cgroup {:?}", self.full_path);
        fs::remove_dir(&self.full_path)?;

        Ok(())
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        let linux_resources = LinuxResources {
            freezer: Some(state),
            ..Default::default()
        };
        Freezer::apply(&linux_resources, &self.full_path)
    }

    fn stats(&self) -> Result<Stats> {
        let mut stats = Stats::default();

        for subsystem in CONTROLLER_TYPES {
            match subsystem {
                ControllerType::Cpu => stats.cpu.usage = Cpu::stats(&self.full_path)?,
                ControllerType::HugeTlb => stats.hugetlb = HugeTlb::stats(&self.full_path)?,
                ControllerType::Pids => stats.pids = Pids::stats(&self.full_path)?,
                ControllerType::Memory => stats.memory = Memory::stats(&self.full_path)?,
                ControllerType::Io => stats.blkio = Io::stats(&self.full_path)?,
                _ => continue,
            }
        }

        Ok(stats)
    }

    fn get_all_pids(&self) -> Result<Vec<Pid>> {
        common::get_all_pids(&self.full_path)
    }
}
