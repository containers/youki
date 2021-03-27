use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use nix::unistd::Pid;
use procfs::process::Process;

use crate::container::{ContainerStatus, State};

#[derive(Debug)]
pub struct Container {
    pub state: State,
    pub root: PathBuf,
}

impl Container {
    pub fn new(
        container_id: &str,
        status: ContainerStatus,
        pid: Option<i32>,
        bundle: &str,
        container_root: &PathBuf,
    ) -> Result<Self> {
        let container_root = fs::canonicalize(container_root)?;
        let state = State::new(container_id, status, pid, bundle);
        Ok(Self {
            state,
            root: container_root,
        })
    }

    pub fn id(&self) -> &str {
        self.state.id.as_str()
    }

    pub fn status(&self) -> ContainerStatus {
        self.state.status
    }

    pub fn refresh_status(&self) -> Result<Self> {
        let new_status = match self.pid() {
            Some(pid) => {
                if let Ok(proc) = Process::new(pid.as_raw()) {
                    use procfs::process::ProcState;
                    match proc.stat.state().unwrap() {
                        ProcState::Zombie | ProcState::Dead => ContainerStatus::Stopped,
                        _ => match self.status() {
                            ContainerStatus::Creating | ContainerStatus::Created => self.status(),
                            _ => ContainerStatus::Running,
                        },
                    }
                } else {
                    ContainerStatus::Stopped
                }
            }
            None => ContainerStatus::Stopped,
        };
        self.update_status(new_status)
    }

    pub fn save(&self) -> Result<()> {
        log::debug!("Sava container status: {:?} in {:?}", self, self.root);
        self.state.save(&self.root)
    }

    pub fn can_start(&self) -> bool {
        self.state.status.can_start()
    }

    pub fn can_kill(&self) -> bool {
        self.state.status.can_kill()
    }

    pub fn can_delete(&self) -> bool {
        self.state.status.can_delete()
    }

    pub fn pid(&self) -> Option<Pid> {
        self.state.pid.map(Pid::from_raw)
    }

    pub fn set_pid(&self, pid: i32) -> Self {
        Self::new(
            self.state.id.as_str(),
            self.state.status,
            Some(pid),
            self.state.bundle.as_str(),
            &self.root,
        )
        .expect("unexpected error")
    }

    pub fn update_status(&self, status: ContainerStatus) -> Result<Self> {
        Self::new(
            self.state.id.as_str(),
            status,
            self.state.pid,
            self.state.bundle.as_str(),
            &self.root,
        )
    }

    pub fn load(container_root: PathBuf) -> Result<Self> {
        let state = State::load(&container_root)?;
        Ok(Self {
            state,
            root: container_root,
        })
    }
}
