use std::collections::HashSet;
use std::fs::{create_dir_all, remove_dir, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use nix::unistd::Pid;
use procfs::process::Process;

use crate::utils::PathBufExt;

pub struct Manager {
    cgroup_path: PathBuf,
}

impl Manager {
    pub fn new(cgroup_path: PathBuf) -> Self {
        Manager { cgroup_path }
    }

    pub fn apply(&self, pid: Pid) -> Result<()> {
        println!("cgroup_path: {:?}", self.cgroup_path.display());
        for mount in Process::myself()?
            .mountinfo()?
            .into_iter()
            .filter(|m| m.fs_type == "cgroup")
        {
            let (a, _b): (HashSet<_>, HashSet<_>) = mount
                .mount_options
                .into_iter()
                .chain(mount.super_options)
                .partition(|&(_, ref m)| m.is_none());

            if mount.mount_point.ends_with("devices") {
                let p = mount.mount_point.join_absolute_path(&self.cgroup_path)?;
                create_dir_all(&p)?;

                eprintln!("pid: {:?}", pid.to_string());

                let cgroups_procs = p.join("cgroup.procs");
                let mut f = OpenOptions::new()
                    .create(false)
                    .write(true)
                    .truncate(true)
                    .open(cgroups_procs)?;
                f.write_all(pid.to_string().as_bytes())?;

                let device_deny = p.join("devices.deny");
                OpenOptions::new()
                    .create(false)
                    .write(true)
                    .truncate(true)
                    .open(device_deny)?
                    .write_all("b 8:0 rw".as_bytes())?;

                println!(
                    "{} on {} type {} ({})",
                    mount.mount_source.unwrap(),
                    mount.mount_point.display(),
                    mount.fs_type,
                    a.into_iter().map(|(k, _)| k).collect::<Vec<_>>().join(",")
                );
            }
        }

        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        for mount in Process::myself()?
            .mountinfo()?
            .into_iter()
            .filter(|m| m.fs_type == "cgroup")
        {
            if mount.mount_point.ends_with("devices") {
                let p = mount.mount_point.join_absolute_path(&self.cgroup_path)?;
                println!("remove p: {:?}", p.display());
                remove_dir(&p)?;
            }
        }

        Ok(())
    }
}
