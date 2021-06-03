use std::{
    env,
    fmt::{Debug, Display},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use nix::unistd::Pid;
use oci_spec::LinuxResources;
use procfs::process::Process;

use crate::cgroups::v1;
use crate::cgroups::v2;

pub const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub trait CgroupManager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()>;
    fn remove(&self) -> Result<()>;
}

#[derive(Debug)]
pub enum Cgroup {
    V1,
    V2,
}

impl Display for Cgroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match *self {
            Cgroup::V1 => "v1",
            Cgroup::V2 => "v2",
        };

        write!(f, "{}", print)
    }
}

#[inline]
pub fn write_cgroup_file<P: AsRef<Path>>(path: P, data: &str) -> Result<()> {
    fs::OpenOptions::new()
        .create(false)
        .write(true)
        .truncate(false)
        .open(path)?
        .write_all(data.as_bytes())?;

    Ok(())
}

pub fn create_cgroup_manager<P: Into<PathBuf>>(cgroup_path: P) -> Result<Box<dyn CgroupManager>> {
    let cgroup_mount = Process::myself()?
        .mountinfo()?
        .into_iter()
        .find(|m| m.fs_type == "cgroup");

    let cgroup2_mount = Process::myself()?
        .mountinfo()?
        .into_iter()
        .find(|m| m.fs_type == "cgroup2");

    match (cgroup_mount, cgroup2_mount) {
        (Some(_), None) => {
            log::info!("cgroup manager V1 will be used");
            Ok(Box::new(v1::manager::Manager::new(cgroup_path.into())?))
        }
        (None, Some(cgroup2)) => {
            log::info!("cgroup manager V2 will be used");
            Ok(Box::new(v2::manager::Manager::new(
                cgroup2.mount_point,
                cgroup_path.into(),
            )?))
        }
        (Some(_), Some(cgroup2)) => {
            let cgroup_override = env::var("YOUKI_PREFER_CGROUPV2");
            match cgroup_override {
                Ok(v) if v == "true" => {
                    log::info!("cgroup manager V2 will be used");
                    Ok(Box::new(v2::manager::Manager::new(
                        cgroup2.mount_point,
                        cgroup_path.into(),
                    )?))
                }
                _ => Ok(Box::new(v1::manager::Manager::new(cgroup_path.into())?)),
            } 
        }
        _ => bail!("could not find cgroup filesystem"),
    }
}
