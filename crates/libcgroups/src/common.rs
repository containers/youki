use std::{
    fmt::{Debug, Display},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf, StripPrefixError},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use nix::{
    sys::statfs::{statfs, CGROUP2_SUPER_MAGIC, TMPFS_MAGIC},
    unistd::Pid,
};
use oci_spec::runtime::LinuxResources;
#[cfg(any(feature = "cgroupsv2_devices", feature = "v1"))]
use oci_spec::runtime::{
    LinuxDevice, LinuxDeviceBuilder, LinuxDeviceCgroup, LinuxDeviceCgroupBuilder, LinuxDeviceType,
};

#[cfg(feature = "systemd")]
use super::systemd;
#[cfg(feature = "v1")]
use super::v1;
#[cfg(feature = "v2")]
use super::v2;

use super::stats::Stats;

pub const CGROUP_PROCS: &str = "cgroup.procs";
pub const DEFAULT_CGROUP_ROOT: &str = "/sys/fs/cgroup";

pub trait CgroupManager {
    /// Adds a task specified by its pid to the cgroup
    fn add_task(&self, pid: Pid) -> Result<()>;

    /// Applies resource restrictions to the cgroup
    fn apply(&self, controller_opt: &ControllerOpt) -> Result<()>;

    /// Removes the cgroup
    fn remove(&self) -> Result<()>;

    /// Sets the freezer cgroup to the specified state
    fn freeze(&self, state: FreezerState) -> Result<()>;

    /// Retrieve statistics for the cgroup
    fn stats(&self) -> Result<Stats>;

    /// Gets the PIDs inside the cgroup
    fn get_all_pids(&self) -> Result<Vec<Pid>>;
}

#[derive(Debug)]
pub enum CgroupSetup {
    Hybrid,
    Legacy,
    Unified,
}

impl Display for CgroupSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match self {
            CgroupSetup::Hybrid => "hybrid",
            CgroupSetup::Legacy => "legacy",
            CgroupSetup::Unified => "unified",
        };

        write!(f, "{print}")
    }
}

/// FreezerState is given freezer contoller
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreezerState {
    /// Tasks in cgroup are undefined
    Undefined,
    /// Tasks in cgroup are suspended.
    Frozen,
    /// Tasks in cgroup are resuming.
    Thawed,
}

/// ControllerOpt is given all cgroup controller for applying cgroup configuration.
#[derive(Clone, Debug)]
pub struct ControllerOpt<'a> {
    /// Resources contain cgroup information for handling resource constraints for the container.
    pub resources: &'a LinuxResources,
    /// Disables the OOM killer for out of memory conditions.
    pub disable_oom_killer: bool,
    /// Specify an oom_score_adj for container.
    pub oom_score_adj: Option<i32>,
    /// FreezerState is given to freezer contoller for suspending process.
    pub freezer_state: Option<FreezerState>,
}

#[derive(thiserror::Error, Debug)]
pub enum WrappedIoError {
    #[error("failed to open {path}: {err}")]
    Open { err: std::io::Error, path: PathBuf },
    #[error("failed to write {data} to {path}: {err}")]
    Write {
        err: std::io::Error,
        path: PathBuf,
        data: String,
    },
    #[error("failed to read {path}: {err}")]
    Read { err: std::io::Error, path: PathBuf },
}

#[inline]
pub fn write_cgroup_file_str<P: AsRef<Path>>(path: P, data: &str) -> Result<(), WrappedIoError> {
    let path = path.as_ref();

    fs::OpenOptions::new()
        .create(false)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|err| WrappedIoError::Open {
            err,
            path: path.to_path_buf(),
        })?
        .write_all(data.as_bytes())
        .map_err(|err| WrappedIoError::Write {
            err,
            path: path.to_path_buf(),
            data: data.into(),
        })?;

    Ok(())
}

#[inline]
pub fn write_cgroup_file<P: AsRef<Path>, T: ToString>(
    path: P,
    data: T,
) -> Result<(), WrappedIoError> {
    let path = path.as_ref();
    let data = data.to_string();

    fs::OpenOptions::new()
        .create(false)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|err| WrappedIoError::Open {
            err,
            path: path.to_path_buf(),
        })?
        .write_all(data.as_bytes())
        .map_err(|err| WrappedIoError::Write {
            err,
            path: path.to_path_buf(),
            data,
        })?;

    Ok(())
}

#[inline]
pub fn read_cgroup_file<P: AsRef<Path>>(path: P) -> Result<String, WrappedIoError> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|err| WrappedIoError::Read {
        err,
        path: path.to_path_buf(),
    })
}

/// Determines the cgroup setup of the system. Systems typically have one of
/// three setups:
/// - Unified: Pure cgroup v2 system.
/// - Legacy: Pure cgroup v1 system.
/// - Hybrid: Hybrid is basically a cgroup v1 system, except for
///   an additional unified hierarchy which doesn't have any
///   controllers attached. Resource control can purely be achieved
///   through the cgroup v1 hierarchy, not through the cgroup v2 hierarchy.
pub fn get_cgroup_setup() -> Result<CgroupSetup> {
    let default_root = Path::new(DEFAULT_CGROUP_ROOT);
    match default_root.exists() {
        true => {
            // If the filesystem is of type cgroup2, the system is in unified mode.
            // If the filesystem is tmpfs instead the system is either in legacy or
            // hybrid mode. If a cgroup2 filesystem has been mounted under the "unified"
            // folder we are in hybrid mode, otherwise we are in legacy mode.
            let stat = statfs(default_root).with_context(|| {
                format!(
                    "failed to stat default cgroup root {}",
                    &default_root.display()
                )
            })?;
            if stat.filesystem_type() == CGROUP2_SUPER_MAGIC {
                return Ok(CgroupSetup::Unified);
            }

            if stat.filesystem_type() == TMPFS_MAGIC {
                let unified = Path::new("/sys/fs/cgroup/unified");
                if Path::new(unified).exists() {
                    let stat = statfs(unified)
                        .with_context(|| format!("failed to stat {}", unified.display()))?;
                    if stat.filesystem_type() == CGROUP2_SUPER_MAGIC {
                        return Ok(CgroupSetup::Hybrid);
                    }
                }

                return Ok(CgroupSetup::Legacy);
            }
        }
        false => bail!("non default cgroup root not supported"),
    }

    bail!("failed to detect cgroup setup");
}

pub fn create_cgroup_manager<P: Into<PathBuf>>(
    cgroup_path: P,
    systemd_cgroup: bool,
    container_name: &str,
) -> Result<Box<dyn CgroupManager>> {
    let cgroup_setup = get_cgroup_setup()?;
    let cgroup_path = cgroup_path.into();

    match cgroup_setup {
        CgroupSetup::Legacy | CgroupSetup::Hybrid => create_v1_cgroup_manager(cgroup_path),
        CgroupSetup::Unified => {
            // ref https://github.com/opencontainers/runtime-spec/blob/main/config-linux.md#cgroups-path
            if cgroup_path.is_absolute() || !systemd_cgroup {
                return create_v2_cgroup_manager(cgroup_path);
            }
            create_systemd_cgroup_manager(cgroup_path, container_name)
        }
    }
}

#[cfg(feature = "v1")]
fn create_v1_cgroup_manager(cgroup_path: PathBuf) -> Result<Box<dyn CgroupManager>> {
    log::info!("cgroup manager V1 will be used");
    Ok(Box::new(v1::manager::Manager::new(cgroup_path)?))
}

#[cfg(not(feature = "v1"))]
fn create_v1_cgroup_manager(_cgroup_path: PathBuf) -> Result<Box<dyn CgroupManager>> {
    bail!("cgroup v1 feature is required, but was not enabled during compile time");
}

#[cfg(feature = "v2")]
fn create_v2_cgroup_manager(cgroup_path: PathBuf) -> Result<Box<dyn CgroupManager>> {
    log::info!("cgroup manager V2 will be used");
    Ok(Box::new(v2::manager::Manager::new(
        DEFAULT_CGROUP_ROOT.into(),
        cgroup_path,
    )?))
}

#[cfg(not(feature = "v2"))]
fn create_v2_cgroup_manager(_cgroup_path: PathBuf) -> Result<Box<dyn CgroupManager>> {
    bail!("cgroup v2 feature is required, but was not enabled during compile time");
}

#[cfg(feature = "systemd")]
fn create_systemd_cgroup_manager(
    cgroup_path: PathBuf,
    container_name: &str,
) -> Result<Box<dyn CgroupManager>> {
    if !systemd::booted() {
        bail!(
            "systemd cgroup flag passed, but systemd support for managing cgroups is not available"
        );
    }

    let use_system = nix::unistd::geteuid().is_root();

    log::info!(
        "systemd cgroup manager with system bus {} will be used",
        use_system
    );
    Ok(Box::new(systemd::manager::Manager::new(
        DEFAULT_CGROUP_ROOT.into(),
        cgroup_path,
        container_name.into(),
        use_system,
    )?))
}

#[cfg(not(feature = "systemd"))]
fn create_systemd_cgroup_manager(
    _cgroup_path: PathBuf,
    _container_name: &str,
) -> Result<Box<dyn CgroupManager>> {
    bail!("systemd cgroup feature is required, but was not enabled during compile time");
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
    fn join_safely<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, JoinSafelyError>;
}

#[derive(thiserror::Error, Debug)]
pub enum JoinSafelyError {
    #[error("failed to strip prefix from {path}: {err}")]
    StripPrefix {
        err: StripPrefixError,
        path: PathBuf,
    },
}

impl PathBufExt for PathBuf {
    fn join_safely<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, JoinSafelyError> {
        let path = path.as_ref();
        if path.is_relative() {
            return Ok(self.join(path));
        }

        let stripped = path
            .strip_prefix("/")
            .map_err(|err| JoinSafelyError::StripPrefix {
                err,
                path: path.to_path_buf(),
            })?;
        Ok(self.join(stripped))
    }
}

#[cfg(any(feature = "cgroupsv2_devices", feature = "v1"))]
pub(crate) fn default_allow_devices() -> Vec<LinuxDeviceCgroup> {
    vec![
        LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::C)
            .access("m")
            .build()
            .unwrap(),
        LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::B)
            .access("m")
            .build()
            .unwrap(),
        // /dev/console
        LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::C)
            .major(5)
            .minor(1)
            .access("rwm")
            .build()
            .unwrap(),
        // /dev/pts
        LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::C)
            .major(136)
            .access("rwm")
            .build()
            .unwrap(),
        LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::C)
            .major(5)
            .minor(2)
            .access("rwm")
            .build()
            .unwrap(),
        // tun/tap
        LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::C)
            .major(10)
            .minor(200)
            .access("rwm")
            .build()
            .unwrap(),
    ]
}

#[cfg(any(feature = "cgroupsv2_devices", feature = "v1"))]
pub(crate) fn default_devices() -> Vec<LinuxDevice> {
    vec![
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/null"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(3)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/zero"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(5)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/full"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(7)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/tty"))
            .typ(LinuxDeviceType::C)
            .major(5)
            .minor(0)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/urandom"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(9)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
        LinuxDeviceBuilder::default()
            .path(PathBuf::from("/dev/random"))
            .typ(LinuxDeviceType::C)
            .major(1)
            .minor(8)
            .file_mode(0o066u32)
            .build()
            .unwrap(),
    ]
}

/// Attempts to delete the path the requested number of times.
pub(crate) fn delete_with_retry<P: AsRef<Path>, L: Into<Option<Duration>>>(
    path: P,
    retries: u32,
    limit_backoff: L,
) -> Result<()> {
    let mut attempts = 0;
    let mut delay = Duration::from_millis(10);
    let path = path.as_ref();
    let limit = limit_backoff.into().unwrap_or(Duration::MAX);

    while attempts < retries {
        if fs::remove_dir(path).is_ok() {
            return Ok(());
        }

        std::thread::sleep(delay);
        attempts += 1;
        delay *= attempts;
        if delay > limit {
            delay = limit;
        }
    }

    bail!("could not delete {:?}", path)
}
