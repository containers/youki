use std::{fmt::Debug, fs, io::Write, path::{Path}};
use nix::sys::statfs;

use anyhow::{Result, anyhow};
use nix::unistd::Pid;

use oci_spec::LinuxResources;

pub const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub trait CgroupManager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()>;
}

#[derive(Debug)]
pub enum Cgroup {
    V1,
    V2,
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

pub fn detect_cgroup_version<P: AsRef<Path> + Debug>(path: P) -> Result<Cgroup> {
    let statfs = statfs::statfs(path.as_ref())?;
    let cgroup_type = match statfs.filesystem_type() {
        statfs::CGROUP_SUPER_MAGIC => Cgroup::V1,
        statfs::CGROUP2_SUPER_MAGIC => Cgroup::V2,
        _ => Err(anyhow!("{:?} is not a cgroup filesystem", path))?,
    };

    Ok(cgroup_type)
}