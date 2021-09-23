use std::{
    env,
    fmt::{Debug, Display},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
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
use tokio_uring::{
    buf::IoBuf,
    fs::OpenOptions,
};

use super::v1;
use super::v2;

use super::stats::Stats;

pub const CGROUP_PROCS: &str = "cgroup.procs";
pub const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";

// This can probably be fine tuned. Any bytes in a buffer not read to are wasted space, but too
// small of a chunk means we need to do more read operations. I baselessly assume 4096 will read in
// most cgroup files in a single operation, and that wasting bytes to kilobytes of memory is worth a
// faster read.
const CHUNK_SIZE: usize = 4096;

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
    write_cgroup_file_str(path, &data.to_string())?;

    Ok(())
}

#[inline]
pub async fn async_write_cgroup_file<P: AsRef<Path>, T: ToString>(path: P, data: T) -> Result<()> {
    async_write_cgroup_file_str(path, &data.to_string()).await?;

    Ok(())
}

#[inline]
pub async fn async_write_cgroup_file_str<P: AsRef<Path>>(path: P, data: &str) -> Result<()> {
    let file = OpenOptions::new()
        .create(false)
        .write(true)
        .truncate(false)
        .open(path.as_ref())
        .await
        .with_context(|| format!("failed to open {:?}", path.as_ref()))?;

    // NOTE: this code won't need to exist once tokio_uring implements write_all_at
    let mut buf = data.as_bytes().to_vec();
    let buf_len = buf.len();
    let mut bytes_written = 0;
    while bytes_written < buf_len {
        let (res, slice) = file
            .write_at(buf.slice(bytes_written..), bytes_written as u64)
            .await;
        buf = slice.into_inner();
        match res {
            Ok(0) => {
                bail!(format!(
                    "failed to write whole buffer to cgroup file: {:?}",
                    path.as_ref()
                ));
            }
            Ok(n) => {
                bytes_written += n;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => bail!(e),
        };
    }

    file.sync_data().await?;

    Ok(())
}

#[inline]
pub fn read_cgroup_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path).with_context(|| format!("failed to open {:?}", path))
}

#[inline]
pub async fn async_read_cgroup_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let file = OpenOptions::new()
        .create(false)
        .read(true)
        .open(path)
        .await?;

    // assuming the chunk size is big enough to read in most files, seems reasonable to just
    // allocate the resulting string to that size
    let mut result = String::with_capacity(CHUNK_SIZE);

    // this is fun... Should probably coordinate with tokio_uring maintainers to see if we can get
    // this added as a feature instead of implementing ourselves
    let buffer = vec![0; CHUNK_SIZE];
    let (res, buffer) = file.read_at(buffer, 0).await;
    let mut bytes_read = res?;
    let s = std::str::from_utf8(&buffer[..bytes_read])?;
    result.push_str(&s);
    // if the amount of bytes read is equal to the chunk size there may be more bytes to read
    while bytes_read == CHUNK_SIZE {
        let buffer = vec![0; CHUNK_SIZE];
        // read the next chunk of bytes
        let (res, buffer) = file.read_at(buffer, bytes_read as u64).await;
        bytes_read = res?;
        // short circuit the read loop, we don't need to do anything if nothing was read
        if bytes_read == 0 {
            break;
        }
        let s = std::str::from_utf8(&buffer[..bytes_read])?;
        result.push_str(&s);
    }

    Ok(result)
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

pub trait PathBufExt {
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
