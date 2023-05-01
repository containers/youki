use std::fs;
use std::path::Path;
use std::time::Duration;
use std::{collections::HashMap, path::PathBuf};

use anyhow::bail;
use anyhow::Result;
use nix::unistd::Pid;

use procfs::process::Process;

use super::{
    blkio::Blkio, controller::Controller, controller_type::CONTROLLERS, cpu::Cpu, cpuacct::CpuAcct,
    cpuset::CpuSet, devices::Devices, freezer::Freezer, hugetlb::HugeTlb, memory::Memory,
    network_classifier::NetworkClassifier, network_priority::NetworkPriority,
    perf_event::PerfEvent, pids::Pids, util, ControllerType as CtrlType,
};

use crate::common::{self, CgroupManager, ControllerOpt, FreezerState, PathBufExt, CGROUP_PROCS};
use crate::stats::{Stats, StatsProvider};

pub struct Manager {
    subsystems: HashMap<CtrlType, PathBuf>,
}

impl Manager {
    /// Constructs a new cgroup manager with cgroups_path being relative to the root of the subsystem
    pub fn new(cgroup_path: PathBuf) -> Result<Self> {
        let mut subsystems = HashMap::<CtrlType, PathBuf>::new();
        for subsystem in CONTROLLERS {
            if let Ok(subsystem_path) = Self::get_subsystem_path(&cgroup_path, subsystem) {
                subsystems.insert(subsystem.clone(), subsystem_path);
            } else {
                log::warn!("cgroup {} not supported on this system", subsystem);
            }
        }

        Ok(Manager { subsystems })
    }

    fn get_subsystem_path(cgroup_path: &Path, subsystem: &CtrlType) -> Result<PathBuf> {
        log::debug!("Get path for subsystem: {}", subsystem);
        let mount_point = util::get_subsystem_mount_point(subsystem)?;

        let cgroup = Process::myself()?
            .cgroups()?
            .into_iter()
            .find(|c| c.controllers.contains(&subsystem.to_string()))
            .unwrap();

        let p = if cgroup_path.as_os_str().is_empty() {
            mount_point.join_safely(Path::new(&cgroup.pathname))?
        } else {
            mount_point.join_safely(cgroup_path)?
        };

        Ok(p)
    }

    fn get_required_controllers(
        &self,
        controller_opt: &ControllerOpt,
    ) -> Result<HashMap<&CtrlType, &PathBuf>> {
        let mut required_controllers = HashMap::new();

        for controller in CONTROLLERS {
            let required = match controller {
                CtrlType::Cpu => Cpu::needs_to_handle(controller_opt).is_some(),
                CtrlType::CpuAcct => CpuAcct::needs_to_handle(controller_opt).is_some(),
                CtrlType::CpuSet => CpuSet::needs_to_handle(controller_opt).is_some(),
                CtrlType::Devices => Devices::needs_to_handle(controller_opt).is_some(),
                CtrlType::HugeTlb => HugeTlb::needs_to_handle(controller_opt).is_some(),
                CtrlType::Memory => Memory::needs_to_handle(controller_opt).is_some(),
                CtrlType::Pids => Pids::needs_to_handle(controller_opt).is_some(),
                CtrlType::PerfEvent => PerfEvent::needs_to_handle(controller_opt).is_some(),
                CtrlType::Blkio => Blkio::needs_to_handle(controller_opt).is_some(),
                CtrlType::NetworkPriority => {
                    NetworkPriority::needs_to_handle(controller_opt).is_some()
                }
                CtrlType::NetworkClassifier => {
                    NetworkClassifier::needs_to_handle(controller_opt).is_some()
                }
                CtrlType::Freezer => Freezer::needs_to_handle(controller_opt).is_some(),
            };

            if required {
                if let Some(subsystem_path) = self.subsystems.get(controller) {
                    required_controllers.insert(controller, subsystem_path);
                } else {
                    bail!("cgroup {} is required to fulfill the request, but is not supported by this system", controller);
                }
            }
        }

        Ok(required_controllers)
    }
}

impl CgroupManager for Manager {
    fn get_all_pids(&self) -> Result<Vec<Pid>> {
        let devices = self.subsystems.get(&CtrlType::Devices);
        if let Some(p) = devices {
            common::get_all_pids(p)
        } else {
            bail!("subsystem does not exist")
        }
    }
    fn add_task(&self, pid: Pid) -> Result<()> {
        for subsys in &self.subsystems {
            match subsys.0 {
                CtrlType::Cpu => Cpu::add_task(pid, subsys.1)?,
                CtrlType::CpuAcct => CpuAcct::add_task(pid, subsys.1)?,
                CtrlType::CpuSet => CpuSet::add_task(pid, subsys.1)?,
                CtrlType::Devices => Devices::add_task(pid, subsys.1)?,
                CtrlType::HugeTlb => HugeTlb::add_task(pid, subsys.1)?,
                CtrlType::Memory => Memory::add_task(pid, subsys.1)?,
                CtrlType::Pids => Pids::add_task(pid, subsys.1)?,
                CtrlType::PerfEvent => PerfEvent::add_task(pid, subsys.1)?,
                CtrlType::Blkio => Blkio::add_task(pid, subsys.1)?,
                CtrlType::NetworkPriority => NetworkPriority::add_task(pid, subsys.1)?,
                CtrlType::NetworkClassifier => NetworkClassifier::add_task(pid, subsys.1)?,
                CtrlType::Freezer => Freezer::add_task(pid, subsys.1)?,
            }
        }

        Ok(())
    }

    fn apply(&self, controller_opt: &ControllerOpt) -> Result<()> {
        for subsys in self.get_required_controllers(controller_opt)? {
            match subsys.0 {
                CtrlType::Cpu => Cpu::apply(controller_opt, subsys.1)?,
                CtrlType::CpuAcct => CpuAcct::apply(controller_opt, subsys.1)?,
                CtrlType::CpuSet => CpuSet::apply(controller_opt, subsys.1)?,
                CtrlType::Devices => Devices::apply(controller_opt, subsys.1)?,
                CtrlType::HugeTlb => HugeTlb::apply(controller_opt, subsys.1)?,
                CtrlType::Memory => Memory::apply(controller_opt, subsys.1)?,
                CtrlType::Pids => Pids::apply(controller_opt, subsys.1)?,
                CtrlType::PerfEvent => PerfEvent::apply(controller_opt, subsys.1)?,
                CtrlType::Blkio => Blkio::apply(controller_opt, subsys.1)?,
                CtrlType::NetworkPriority => NetworkPriority::apply(controller_opt, subsys.1)?,
                CtrlType::NetworkClassifier => NetworkClassifier::apply(controller_opt, subsys.1)?,
                CtrlType::Freezer => Freezer::apply(controller_opt, subsys.1)?,
            }
        }

        Ok(())
    }

    fn remove(&self) -> Result<()> {
        for cgroup_path in &self.subsystems {
            if cgroup_path.1.exists() {
                log::debug!("remove cgroup {:?}", cgroup_path.1);
                let procs_path = cgroup_path.1.join(CGROUP_PROCS);
                let procs = fs::read_to_string(procs_path)?;

                for line in procs.lines() {
                    let pid: i32 = line.parse()?;
                    let _ = nix::sys::signal::kill(Pid::from_raw(pid), nix::sys::signal::SIGKILL);
                }

                common::delete_with_retry(cgroup_path.1, 4, Duration::from_millis(100))?;
            }
        }

        Ok(())
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        let controller_opt = ControllerOpt {
            resources: &Default::default(),
            freezer_state: Some(state),
            oom_score_adj: None,
            disable_oom_killer: false,
        };
        Freezer::apply(
            &controller_opt,
            self.subsystems.get(&CtrlType::Freezer).unwrap(),
        )
    }

    fn stats(&self) -> Result<Stats> {
        let mut stats = Stats::default();

        for subsystem in &self.subsystems {
            match subsystem.0 {
                CtrlType::Cpu => stats.cpu.throttling = Cpu::stats(subsystem.1)?,
                CtrlType::CpuAcct => stats.cpu.usage = CpuAcct::stats(subsystem.1)?,
                CtrlType::Pids => stats.pids = Pids::stats(subsystem.1)?,
                CtrlType::HugeTlb => stats.hugetlb = HugeTlb::stats(subsystem.1)?,
                CtrlType::Blkio => stats.blkio = Blkio::stats(subsystem.1)?,
                CtrlType::Memory => stats.memory = Memory::stats(subsystem.1)?,
                _ => continue,
            }
        }

        Ok(stats)
    }
}
