use std::{
    env,
    fmt::{Debug, Display},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use nix::unistd::Pid;
use oci_spec::{FreezerState, LinuxResources};
use procfs::process::Process;
#[cfg(feature = "systemd_cgroups")]
use systemd::daemon::booted;
#[cfg(not(feature = "systemd_cgroups"))]
fn booted() -> Result<bool> {
    bail!("This build does not include the systemd cgroups feature")
}

use crate::cgroups::v1;
use crate::cgroups::v2;

use super::stats::Stats;

pub const CGROUP_PROCS: &str = "cgroup.procs";
pub const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub trait CgroupManager {
    /// Adds a task specified by its pid to the cgroup
    fn add_task(&self, pid: Pid) -> Result<()>;
    /// Applies resource restrictions to the cgroup
    fn apply(&self, linux_resources: &LinuxResources) -> Result<()>;
    /// Removes the cgroup
    fn remove(&self) -> Result<()>;
    // Sets the freezer cgroup to the specified state
    fn freeze(&self, state: FreezerState) -> Result<()>;
    /// Retrieve statistics for the cgroup
    fn stats(&self) -> Result<Stats>;
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
pub fn write_cgroup_file_str<P: AsRef<Path>>(path: P, data: &str) -> Result<()> {
    fs::OpenOptions::new()
        .create(false)
        .write(true)
        .truncate(false)
        .open(path.as_ref())
        .with_context(|| format!("failed to open {:?}", path.as_ref()))?
        .write_all(data.as_bytes())
        .with_context(|| format!("failed to write to {:?}", path.as_ref()))?;

    Ok(())
}

#[inline]
pub fn write_cgroup_file<P: AsRef<Path>, T: ToString>(path: P, data: T) -> Result<()> {
    fs::OpenOptions::new()
        .create(false)
        .write(true)
        .truncate(false)
        .open(path.as_ref())
        .with_context(|| format!("failed to open {:?}", path.as_ref()))?
        .write_all(data.to_string().as_bytes())
        .with_context(|| format!("failed to write to {:?}", path.as_ref()))?;

    Ok(())
}

#[inline]
pub fn read_cgroup_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path).with_context(|| format!("failed to open {:?}", path))
}

pub fn get_supported_cgroup_fs() -> Result<Vec<Cgroup>> {
    let cgroup_mount = Process::myself()?
        .mountinfo()?
        .into_iter()
        .find(|m| m.fs_type == "cgroup");

    let cgroup2_mount = Process::myself()?
        .mountinfo()?
        .into_iter()
        .find(|m| m.fs_type == "cgroup2");

    let mut cgroups = vec![];
    if cgroup_mount.is_some() {
        cgroups.push(Cgroup::V1);
    }

    if cgroup2_mount.is_some() {
        cgroups.push(Cgroup::V2);
    }

    Ok(cgroups)
}

pub fn create_cgroup_manager<P: Into<PathBuf>>(
    cgroup_path: P,
    systemd_cgroup: bool,
) -> Result<Box<dyn CgroupManager>> {
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
            if systemd_cgroup {
                if !booted()? {
                    bail!("systemd cgroup flag passed, but systemd support for managing cgroups is not available");
                }
                log::info!("systemd cgroup manager will be used");
                return Ok(Box::new(v2::SystemDCGroupManager::new(
                    cgroup2.mount_point,
                    cgroup_path.into(),
                )?));
            }
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
                    if systemd_cgroup {
                        if !booted()? {
                            bail!("systemd cgroup flag passed, but systemd support for managing cgroups is not available");
                        }
                        log::info!("systemd cgroup manager will be used");
                        return Ok(Box::new(v2::SystemDCGroupManager::new(
                            cgroup2.mount_point,
                            cgroup_path.into(),
                        )?));
                    }
                    Ok(Box::new(v2::manager::Manager::new(
                        cgroup2.mount_point,
                        cgroup_path.into(),
                    )?))
                }
                _ => {
                    log::info!("cgroup manager V1 will be used");
                    Ok(Box::new(v1::manager::Manager::new(cgroup_path.into())?))
                }
            }
        }
        _ => bail!("could not find cgroup filesystem"),
    }
}
