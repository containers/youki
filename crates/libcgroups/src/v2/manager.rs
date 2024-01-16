use std::{
    fs::{self},
    os::unix::fs::PermissionsExt,
    path::{Component::RootDir, Path, PathBuf},
    time::Duration,
};

use nix::unistd::Pid;

#[cfg(feature = "cgroupsv2_devices")]
use super::devices::Devices;
use super::{
    controller::Controller,
    controller_type::{
        ControllerType, PseudoControllerType, CONTROLLER_TYPES, PSEUDO_CONTROLLER_TYPES,
    },
    cpu::{Cpu, V2CpuControllerError, V2CpuStatsError},
    cpuset::CpuSet,
    freezer::{Freezer, V2FreezerError},
    hugetlb::{HugeTlb, V2HugeTlbControllerError, V2HugeTlbStatsError},
    io::{Io, V2IoControllerError, V2IoStatsError},
    memory::{Memory, V2MemoryControllerError, V2MemoryStatsError},
    pids::Pids,
    unified::{Unified, V2UnifiedError},
    util::{self, V2UtilError, CGROUP_SUBTREE_CONTROL},
};
use crate::{
    common::{
        self, AnyCgroupManager, CgroupManager, ControllerOpt, FreezerState, JoinSafelyError,
        PathBufExt, WrapIoResult, WrappedIoError, CGROUP_PROCS,
    },
    stats::{PidStatsError, Stats, StatsProvider},
};

pub const CGROUP_KILL: &str = "cgroup.kill";

#[derive(thiserror::Error, Debug)]
pub enum V2ManagerError {
    #[error("io error: {0}")]
    WrappedIo(#[from] WrappedIoError),
    #[error("while joining paths: {0}")]
    JoinSafely(#[from] JoinSafelyError),
    #[error(transparent)]
    Util(#[from] V2UtilError),

    #[error(transparent)]
    CpuController(#[from] V2CpuControllerError),
    #[error(transparent)]
    CpuSetController(WrappedIoError),
    #[error(transparent)]
    HugeTlbController(#[from] V2HugeTlbControllerError),
    #[error(transparent)]
    IoController(#[from] V2IoControllerError),
    #[error(transparent)]
    MemoryController(#[from] V2MemoryControllerError),
    #[error(transparent)]
    PidsController(WrappedIoError),
    #[error(transparent)]
    UnifiedController(#[from] V2UnifiedError),
    #[error(transparent)]
    FreezerController(#[from] V2FreezerError),
    #[cfg(feature = "cgroupsv2_devices")]
    #[error(transparent)]
    DevicesController(#[from] super::devices::controller::DevicesControllerError),

    #[error(transparent)]
    CpuStats(#[from] V2CpuStatsError),
    #[error(transparent)]
    HugeTlbStats(#[from] V2HugeTlbStatsError),
    #[error(transparent)]
    PidsStats(PidStatsError),
    #[error(transparent)]
    MemoryStats(#[from] V2MemoryStatsError),
    #[error(transparent)]
    IoStats(#[from] V2IoStatsError),
}

/// Represents a management interface for a cgroup located at `{root_path}/{cgroup_path}`
///
/// This struct does not have ownership of the cgroup
pub struct Manager {
    root_path: PathBuf,
    cgroup_path: PathBuf,
    full_path: PathBuf,
}

impl Manager {
    /// Constructs a new cgroup manager with root path being the mount point
    /// of a cgroup v2 fs and cgroup path being a relative path from the root
    pub fn new(root_path: PathBuf, cgroup_path: PathBuf) -> Result<Self, V2ManagerError> {
        let full_path = root_path.join_safely(&cgroup_path)?;

        Ok(Self {
            root_path,
            cgroup_path,
            full_path,
        })
    }

    /// Creates a unified cgroup at `self.full_path` and attaches a process to it
    fn create_unified_cgroup(&self, pid: Pid) -> Result<(), V2ManagerError> {
        let controllers: Vec<String> = util::get_available_controllers(&self.root_path)?
            .iter()
            .map(|c| format!("+{c}"))
            .collect();

        Self::write_controllers(&self.root_path, &controllers)?;

        let mut current_path = self.root_path.clone();
        let mut components = self
            .cgroup_path
            .components()
            .filter(|c| c.ne(&RootDir))
            .peekable();
        while let Some(component) = components.next() {
            current_path = current_path.join(component);
            if !current_path.exists() {
                fs::create_dir(&current_path).wrap_create_dir(&current_path)?;
                fs::metadata(&current_path)
                    .wrap_other(&current_path)?
                    .permissions()
                    .set_mode(0o755);
            }

            // last component cannot have subtree_control enabled due to internal process constraint
            // if this were set, writing to the cgroups.procs file will fail with Erno 16 (device or resource busy)
            if components.peek().is_some() {
                Self::write_controllers(&current_path, &controllers)?;
            }
        }

        common::write_cgroup_file(self.full_path.join(CGROUP_PROCS), pid)?;
        Ok(())
    }

    /// Writes a list of controllers to the `{path}/cgroup.subtree_control` file
    fn write_controllers(path: &Path, controllers: &[String]) -> Result<(), WrappedIoError> {
        for controller in controllers {
            common::write_cgroup_file_str(path.join(CGROUP_SUBTREE_CONTROL), controller)?;
        }

        Ok(())
    }

    pub fn any(self) -> AnyCgroupManager {
        AnyCgroupManager::V2(self)
    }
}

impl CgroupManager for Manager {
    type Error = V2ManagerError;

    fn add_task(&self, pid: Pid) -> Result<(), Self::Error> {
        self.create_unified_cgroup(pid)?;
        Ok(())
    }

    fn apply(&self, controller_opt: &ControllerOpt) -> Result<(), Self::Error> {
        for controller in CONTROLLER_TYPES {
            match controller {
                ControllerType::Cpu => Cpu::apply(controller_opt, &self.full_path)?,
                ControllerType::CpuSet => CpuSet::apply(controller_opt, &self.full_path)?,
                ControllerType::HugeTlb => HugeTlb::apply(controller_opt, &self.full_path)?,
                ControllerType::Io => Io::apply(controller_opt, &self.full_path)?,
                ControllerType::Memory => Memory::apply(controller_opt, &self.full_path)?,
                ControllerType::Pids => Pids::apply(controller_opt, &self.full_path)?,
            }
        }

        #[cfg(feature = "cgroupsv2_devices")]
        Devices::apply(controller_opt, &self.cgroup_path)?;

        for pseudoctlr in PSEUDO_CONTROLLER_TYPES {
            if let PseudoControllerType::Unified = pseudoctlr {
                Unified::apply(
                    controller_opt,
                    &self.full_path,
                    util::get_available_controllers(&self.root_path)?,
                )?;
            }
        }

        Ok(())
    }

    fn remove(&self) -> Result<(), Self::Error> {
        if self.full_path.exists() {
            tracing::debug!("remove cgroup {:?}", self.full_path);
            let kill_file = self.full_path.join(CGROUP_KILL);
            if kill_file.exists() {
                fs::write(&kill_file, "1").wrap_write(&kill_file, "1")?;
            } else {
                let procs_path = self.full_path.join(CGROUP_PROCS);
                let procs = fs::read_to_string(&procs_path).wrap_read(&procs_path)?;

                for line in procs.lines() {
                    let pid: i32 = line
                        .parse()
                        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
                        .wrap_other(&procs_path)?;
                    let _ = nix::sys::signal::kill(Pid::from_raw(pid), nix::sys::signal::SIGKILL);
                }
            }

            common::delete_with_retry(&self.full_path, 4, Duration::from_millis(100))?;
        }

        Ok(())
    }

    fn freeze(&self, state: FreezerState) -> Result<(), Self::Error> {
        let controller_opt = ControllerOpt {
            resources: &Default::default(),
            freezer_state: Some(state),
            oom_score_adj: None,
            disable_oom_killer: false,
        };
        Ok(Freezer::apply(&controller_opt, &self.full_path)?)
    }

    fn stats(&self) -> Result<Stats, Self::Error> {
        let mut stats = Stats::default();

        for subsystem in CONTROLLER_TYPES {
            match subsystem {
                ControllerType::Cpu => stats.cpu = Cpu::stats(&self.full_path)?,
                ControllerType::HugeTlb => stats.hugetlb = HugeTlb::stats(&self.full_path)?,
                ControllerType::Pids => {
                    stats.pids = Pids::stats(&self.full_path).map_err(V2ManagerError::PidsStats)?
                }
                ControllerType::Memory => stats.memory = Memory::stats(&self.full_path)?,
                ControllerType::Io => stats.blkio = Io::stats(&self.full_path)?,
                _ => continue,
            }
        }

        Ok(stats)
    }

    fn get_all_pids(&self) -> Result<Vec<Pid>, Self::Error> {
        Ok(common::get_all_pids(&self.full_path)?)
    }
}
