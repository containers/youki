use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use procfs::process::Process;

use crate::common;

use super::controller_type::ControllerType;

pub const CGROUP_CONTROLLERS: &str = "cgroup.controllers";
pub const CGROUP_SUBTREE_CONTROL: &str = "cgroup.subtree_control";

pub fn get_unified_mount_point() -> Result<PathBuf> {
    Process::myself()?
        .mountinfo()?
        .into_iter()
        .find(|m| m.fs_type == "cgroup2")
        .map(|m| m.mount_point)
        .ok_or_else(|| anyhow!("could not find mountpoint for unified"))
}

pub fn get_available_controllers(root_path: &Path) -> Result<Vec<ControllerType>> {
    let controllers_path = root_path.join(CGROUP_CONTROLLERS);
    if !controllers_path.exists() {
        bail!(
            "cannot get available controllers. {:?} does not exist",
            controllers_path
        )
    }

    let mut controllers = Vec::new();
    for controller in common::read_cgroup_file(controllers_path)?.split_whitespace() {
        match controller {
            "cpu" => controllers.push(ControllerType::Cpu),
            "cpuset" => controllers.push(ControllerType::CpuSet),
            "hugetlb" => controllers.push(ControllerType::HugeTlb),
            "io" => controllers.push(ControllerType::Io),
            "memory" => controllers.push(ControllerType::Memory),
            "pids" => controllers.push(ControllerType::Pids),
            tpe => log::warn!("Controller {} is not yet implemented.", tpe),
        }
    }

    Ok(controllers)
}
