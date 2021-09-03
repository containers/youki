use std::{
    env,
    fmt::{Debug, Display},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use nix::unistd::Pid;
use oci_spec::{FreezerState, LinuxDevice, LinuxDeviceCgroup, LinuxDeviceType, LinuxResources};
use procfs::process::Process;
#[cfg(feature = "systemd_cgroups")]
use systemd::daemon::booted;
#[cfg(not(feature = "systemd_cgroups"))]
fn booted() -> Result<bool> {
    bail!("This build does not include the systemd cgroups feature")
}

use super::v1;
use super::v2;

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
    // Gets the PIDs inside the cgroup
    fn get_all_pids(&self) -> Result<Vec<Pid>>;
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

pub fn get_all_pids(path: &Path) -> Result<Vec<Pid>> {
    log::debug!("scan pids in folder: {:?}", path);
    let mut result = vec![];
    walk_dir(path, &mut |p| {
        let file_path = p.join(CGROUP_PROCS);
        if file_path.exists() {
            let file = File::open(file_path)?;
            for line in BufReader::new(file).lines().flatten() {
                result.push(Pid::from_raw(line.parse::<i32>()?))
            }
        }
        Ok(())
    })?;
    Ok(result)
}

fn walk_dir<F>(path: &Path, c: &mut F) -> Result<()>
where
    F: FnMut(&Path) -> Result<()>,
{
    c(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            walk_dir(&path, c)?;
        }
    }
    Ok(())
}

pub(crate) trait PathBufExt {
    fn join_safely(&self, p: &Path) -> Result<PathBuf>;
}

impl PathBufExt for PathBuf {
    fn join_safely(&self, p: &Path) -> Result<PathBuf> {
        if !p.is_absolute() && !p.as_os_str().is_empty() {
            bail!(
                "cannot join {:?} because it is not the absolute path.",
                p.display()
            )
        }
        Ok(PathBuf::from(format!("{}{}", self.display(), p.display())))
    }
}

pub(crate) fn default_allow_devices() -> Vec<LinuxDeviceCgroup> {
    vec![
        LinuxDeviceCgroup {
            allow: true,
            typ: Some(LinuxDeviceType::C),
            major: None,
            minor: None,
            access: "m".to_string().into(),
        },
        LinuxDeviceCgroup {
            allow: true,
            typ: Some(LinuxDeviceType::B),
            major: None,
            minor: None,
            access: "m".to_string().into(),
        },
        // /dev/console
        LinuxDeviceCgroup {
            allow: true,
            typ: Some(LinuxDeviceType::C),
            major: Some(5),
            minor: Some(1),
            access: "rwm".to_string().into(),
        },
        // /dev/pts
        LinuxDeviceCgroup {
            allow: true,
            typ: Some(LinuxDeviceType::C),
            major: Some(136),
            minor: None,
            access: "rwm".to_string().into(),
        },
        LinuxDeviceCgroup {
            allow: true,
            typ: Some(LinuxDeviceType::C),
            major: Some(5),
            minor: Some(2),
            access: "rwm".to_string().into(),
        },
        // tun/tap
        LinuxDeviceCgroup {
            allow: true,
            typ: Some(LinuxDeviceType::C),
            major: Some(10),
            minor: Some(200),
            access: "rwm".to_string().into(),
        },
    ]
}

pub(crate) fn default_devices() -> Vec<LinuxDevice> {
    vec![
        LinuxDevice {
            path: PathBuf::from("/dev/null"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 3,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/zero"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 5,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/full"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 7,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/tty"),
            typ: LinuxDeviceType::C,
            major: 5,
            minor: 0,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/urandom"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 9,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
        LinuxDevice {
            path: PathBuf::from("/dev/random"),
            typ: LinuxDeviceType::C,
            major: 1,
            minor: 8,
            file_mode: Some(0o066),
            uid: None,
            gid: None,
        },
    ]
}
