use std::{collections::HashMap, path::PathBuf};
use std::{fs::remove_dir, path::Path};

use anyhow::Result;
use nix::unistd::Pid;
use procfs::process::Process;

use crate::{cgroups::ControllerType, spec::LinuxResources, utils::PathBufExt};

use super::{devices::Devices, hugetlb::Hugetlb, memory::Memory, pids::Pids, Controller};

const CONTROLLERS: &[ControllerType] = &[
    ControllerType::Devices,
    ControllerType::HugeTlb,
    ControllerType::Memory,
    ControllerType::Pids,
];

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

    pub fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()> {
        for subsys in &self.subsystems {
            match subsys.0.as_str() {
                "devices" => Devices::apply(linux_resources, &subsys.1, pid)?,
                "hugetlb" => Hugetlb::apply(linux_resources, &subsys.1, pid)?,
                "memory" => Memory::apply(linux_resources, &subsys.1, pid)?,
                "pids" => Pids::apply(linux_resources, &subsys.1, pid)?,
                _ => continue,
            }
        }

        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        for cgroup_path in &self.subsystems {
            if cgroup_path.1.exists() {
                log::debug!("remove cgroup {:?}", cgroup_path.1);
                remove_dir(&cgroup_path.1)?;
            }
        }

        Ok(())
    }

    fn get_subsystem_path(cgroup_path: &Path, subsystem: &str) -> anyhow::Result<PathBuf> {
        let mount = Process::myself()?
            .mountinfo()?
            .into_iter()
            .filter(|m| m.fs_type == "cgroup" && m.mount_point.ends_with(subsystem))
            .collect::<Vec<_>>()
            .pop()
            .unwrap();

        let cgroup = Process::myself()?
            .cgroups()?
            .into_iter()
            .filter(|c| c.controllers.contains(&subsystem.to_owned()))
            .collect::<Vec<_>>()
            .pop()
            .unwrap();

        let p = if cgroup_path.to_string_lossy().into_owned().is_empty() {
            mount
                .mount_point
                .join_absolute_path(Path::new(&cgroup.pathname))?
        } else {
            mount.mount_point.join_absolute_path(&cgroup_path)?
        };

        Ok(p)
    }
}
