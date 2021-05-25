use std::{fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}};
use anyhow::{Result, anyhow};

use nix::unistd::Pid;
use oci_spec::LinuxResources;

use crate::{cgroups::v2::controller::Controller, cgroups::{common::CgroupManager, v2::controller_type::ControllerType}};
use super::{cpu::Cpu, cpuset::CpuSet, memory::Memory};

const CGROUP2_MAGIC_NUMBER:u32  = 0x63677270;
const CGROUP_CONTROLLERS: &str = "cgroup.controllers";

const ControllerTypes: &[ControllerType] = &[ControllerType::Cpu, ControllerType::CpuSet, ControllerType::Memory];

pub struct Manager {
   root_path: PathBuf,
}

impl Manager {
    fn new(root_path: PathBuf) -> Result<Self> {
        Ok(Self {
            root_path,
        })
    }

    pub fn remove(&self, cgroup_path: &Path) -> Result<()> {
        let full_path = self.root_path.join(cgroup_path);
        fs::remove_dir_all(full_path)?;

        Ok(())
    }

    fn create_unified_cgroup(&self, cgroup_path: &Path, pid: Pid) -> Result<()> {
        fs::create_dir_all(cgroup_path)?;

        OpenOptions::new()
        .write(true)
        .open(cgroup_path)?
        .write_all(pid.to_string().as_bytes())?;

        Ok(())
    }

    fn get_available_controllers(&self, cgroup_path: &Path) -> Result<Vec<ControllerType>> {
        let controllers_path = self.root_path.join(cgroup_path).join(CGROUP_CONTROLLERS);
        if controllers_path.exists() {
            return Err(anyhow!(""));
        }
        
        let mut controllers = Vec::new();
        for controller in fs::read_to_string(&controllers_path)?.split_whitespace() {
            match controller {
                "cpu" => controllers.push(ControllerType::Cpu),
                "cpuset" => controllers.push(ControllerType::CpuSet),
                "io" => controllers.push(ControllerType::IO),
                "memory" => controllers.push(ControllerType::Memory),
                "pids" => controllers.push(ControllerType::Pids),
                "hugetlb" => controllers.push(ControllerType::HugeTlb),
                _ => continue,
            }
        }

        Ok(controllers)
    }

    fn get_required_controllers(&self, cgroup_path: &Path, resources: &LinuxResources) -> Result<Vec<ControllerType>> {
        todo!();
    }
}

impl CgroupManager for Manager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()> {
        self.create_unified_cgroup(&self.root_path, pid)?;

        for controller in ControllerTypes {
            match controller {
                &ControllerType::Cpu => Cpu::apply(linux_resources, &self.root_path)?,
                &ControllerType::CpuSet => CpuSet::apply(linux_resources, &self.root_path)?,
                &ControllerType::Memory => Memory::apply(linux_resources, &self.root_path)?,
                _ => continue,
            }
        }

        Ok(())
    }
}

