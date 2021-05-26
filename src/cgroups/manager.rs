use std::{collections::HashMap, path::PathBuf};
use std::{fs::remove_dir, path::Path};
use futures::future::join_all;

use anyhow::Result;
use nix::unistd::Pid;
use procfs::process::Process;

use crate::{cgroups::ControllerType, spec::LinuxResources, utils::PathBufExt};

use super::{
    blkio::Blkio, devices::Devices, hugetlb::Hugetlb, memory::Memory,
    network_classifier::NetworkClassifier, network_priority::NetworkPriority, pids::Pids,
    Controller,
};

const CONTROLLERS: &[ControllerType] = &[
    ControllerType::Devices,
    ControllerType::HugeTlb,
    ControllerType::Memory,
    ControllerType::Pids,
    ControllerType::Blkio,
    ControllerType::NetworkPriority,
    ControllerType::NetworkClassifier,
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
        smol::block_on(async {
            // let futures = Vec::with_capacity(7);
            // for subsys in &self.subsystems {
            //     futures.push(match subsys.0.as_str() {
            //         "devices" => Devices::apply(linux_resources, &subsys.1, pid),
            //         "hugetlb" => Hugetlb::apply(linux_resources, &subsys.1, pid),
            //         "memory" => Memory::apply(linux_resources, &subsys.1, pid),
            //         "pids" => Pids::apply(linux_resources, &subsys.1, pid),
            //         "blkio" => Blkio::apply(linux_resources, &subsys.1, pid),
            //         "net_prio" => NetworkPriority::apply(linux_resources, &subsys.1, pid),
            //         "net_cls" => NetworkClassifier::apply(linux_resources, &subsys.1, pid),
            //         _ => continue,
            //     });
            // }

            let futures = self.subsystems.iter()
                .filter_map(|entry| {
                    let key = entry.0.as_str();
                    let value = entry.1;
                    match key {
                        "devices" => Some(Devices::apply(linux_resources, value, pid)),
                        "hugetlb" => Some(Hugetlb::apply(linux_resources, value, pid)),
                        "memory" => Some(Memory::apply(linux_resources, value, pid)),
                        "pids" => Some(Pids::apply(linux_resources, value, pid)),
                        "blkio" => Some(Blkio::apply(linux_resources, value, pid)),
                        "net_prio" => Some(NetworkPriority::apply(linux_resources, value, pid)),
                        "net_cls" => Some(NetworkClassifier::apply(linux_resources, value, pid)),
                        _ => None,
                    }
                }).collect::<Vec<_>>();

            join_all(futures).await.iter()
                .for_each(|result| {
                    result.as_ref().expect("Cgroup controller future returned a failure");
                });

            Ok(())
        })
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
        log::debug!("Get path for subsystem: {}", subsystem);
        let mount = Process::myself()?
            .mountinfo()?
            .into_iter()
            .find(|m| {
                if m.fs_type == "cgroup" {
                    // Some systems mount net_prio and net_cls in the same directory
                    // other systems mount them in their own diretories. This
                    // should handle both cases.
                    if subsystem == "net_cls" {
                        return m.mount_point.ends_with("net_cls,net_prio")
                            || m.mount_point.ends_with("net_prio,net_cls")
                            || m.mount_point.ends_with("net_cls");
                    } else if subsystem == "net_prio" {
                        return m.mount_point.ends_with("net_cls,net_prio")
                            || m.mount_point.ends_with("net_prio,net_cls")
                            || m.mount_point.ends_with("net_prio");
                    }
                    return m.mount_point.ends_with(subsystem);
                }
                return false;
            })
            .expect("Failed to find mount point for subsystem");

        let cgroup = Process::myself()?
            .cgroups()?
            .into_iter()
            .find(|c| c.controllers.contains(&subsystem.to_owned()))
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
