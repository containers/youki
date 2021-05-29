use std::{fmt::{Debug, Display}, fs, io::Write, path::{Path, PathBuf}};
use nix::sys::statfs;

use anyhow::{Result, anyhow};
use nix::unistd::Pid;
use procfs::process::Process;
use oci_spec::LinuxResources;

use crate::cgroups::v1;
use crate::cgroups::v2;


pub const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub trait CgroupManager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()>;
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

pub fn write_cgroup_file_truncate(path: &Path, data: &str) -> Result<()> {
    fs::OpenOptions::new()
    .create(false)
    .write(true)
    .truncate(true)
    .open(path)?
    .write_all(data.as_bytes())?;

    Ok(())
}

pub fn write_cgroup_file(path: &Path, data: &str) -> Result<()> {
    fs::OpenOptions::new()
    .create(false)
    .write(true)
    .truncate(false)
    .open(path)?
    .write_all(data.as_bytes())?;

    Ok(())
}

pub fn detect_cgroup_version<P: AsRef<Path> + Debug>(path: P) -> Result<Option<Cgroup>> {
    let statfs = statfs::statfs(path.as_ref())?;
    let cgroup_version = match statfs.filesystem_type() {
        statfs::CGROUP_SUPER_MAGIC => Some(Cgroup::V1),
        statfs::CGROUP2_SUPER_MAGIC => Some(Cgroup::V2),
        _ => None
    };

    Ok(cgroup_version)
}

pub fn create_cgroup_manager<P: Into<PathBuf>>(cgroup_path: P) -> Result<Box<dyn CgroupManager>> {
    // first try the usual cgroup fs location
    let root_cgroup_path = PathBuf::from(DEFAULT_CGROUP_ROOT);
    if root_cgroup_path.exists() {
        if let Some(cgroup_version) = detect_cgroup_version(&root_cgroup_path)? {
            log::info!("cgroup manager {} will be used", cgroup_version);
            let manager: Box<dyn CgroupManager> = match cgroup_version {
                Cgroup::V1 => Box::new(v1::manager::Manager::new(cgroup_path.into())?),
                Cgroup::V2 => Box::new(v2::manager::Manager::new(root_cgroup_path, cgroup_path.into())?),
            };

            return Ok(manager);
        }
    }

    // try to find it from the mountinfo
    let mount = Process::myself()?
    .mountinfo()?
    .into_iter()
    .find(|m| m.fs_type == "cgroup2");

    if let Some(cgroup2) = mount {
        log::info!("cgroup manager V2 will be used");
        return Ok(Box::new(v2::manager::Manager::new(cgroup2.mount_point, cgroup_path.into())?));
    }

    let mount = Process::myself()?
    .mountinfo()?
    .into_iter()
    .find(|m| m.fs_type == "cgroup");

    if let Some(_) = mount {
        log::info!("cgroup manager V1 will be used");
        return Ok(Box::new(v1::manager::Manager::new(cgroup_path.into())?));
    }
    
    Err(anyhow!("could not find cgroup filesystem"))
}