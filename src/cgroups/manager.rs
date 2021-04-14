use std::path::PathBuf;
use std::{fs::remove_dir, path::Path};

use anyhow::Result;
use nix::unistd::Pid;
use procfs::process::{MountInfo, Process};

use crate::{cgroups::ControllerType, spec::LinuxResources, utils::PathBufExt};

use super::{devices::Devices, Controller};

pub struct Manager {
    cgroup_path: PathBuf,
    mount_info: MountInfo,
}

impl Manager {
    pub fn new(cgroup_path: PathBuf) -> Result<Self> {
        let mut mount: Vec<MountInfo> = Process::myself()?
            .mountinfo()?
            .into_iter()
            .filter(|m| {
                m.fs_type == "cgroup"
                    && m.mount_point.ends_with(ControllerType::Devices.to_string())
            })
            .collect();
        Ok(Manager {
            cgroup_path,
            mount_info: mount.pop().unwrap(),
        })
    }

    pub fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()> {
        let cgroup = Process::myself()?
            .cgroups()?
            .into_iter()
            .filter(|c| c.controllers.contains(&ControllerType::Devices.to_string()))
            .collect::<Vec<_>>()
            .pop()
            .unwrap();

        let p = if self.cgroup_path.to_string_lossy().into_owned().is_empty() {
            self.mount_info
                .mount_point
                .join_absolute_path(Path::new(&cgroup.pathname))?
        } else {
            self.mount_info
                .mount_point
                .join_absolute_path(&self.cgroup_path)?
        };

        Devices::apply(linux_resources, &p, pid)?;

        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        let p = self
            .mount_info
            .mount_point
            .join_absolute_path(&self.cgroup_path)?;
        println!("remove_dir: {:?}", p.display());
        remove_dir(&p)?;

        Ok(())
    }
}
