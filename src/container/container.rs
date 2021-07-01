use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::DateTime;
use nix::unistd::Pid;

use chrono::Utc;
use procfs::process::Process;

use crate::command::syscall::create_syscall;

use crate::container::{ContainerStatus, State};

/// Structure representing the container data
#[derive(Debug)]
pub struct Container {
    // State of the container
    pub state: State,
    // indicated the directory for the root path in the container
    pub root: PathBuf,
}

impl Container {
    pub fn new(
        container_id: &str,
        status: ContainerStatus,
        pid: Option<i32>,
        bundle: &str,
        container_root: &Path,
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
    pub fn refresh_status(&mut self) -> Result<Self> {
        let new_status = match self.pid() {
            Some(pid) => {
                // Note that Process::new does not spawn a new process
                // but instead creates a new Process structure, and fill
                // it with information about the process with given pid
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
        Ok(self.update_status(new_status))
    }

    pub fn refresh_state(&self) -> Result<Self> {
        let state = State::load(&self.root)?;
        Ok(Self {
            state,
            root: self.root.clone(),
        })
    }

    pub fn save(&self) -> Result<()> {
        log::debug!("Save container status: {:?} in {:?}", self, self.root);
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
        let mut new_state = self.state.clone();
        new_state.pid = Some(pid);

        Self {
            state: new_state,
            root: self.root.clone(),
        }
    }

    pub fn created(&self) -> Option<DateTime<Utc>> {
        self.state.created
    }

    pub fn set_creator(mut self, uid: u32) -> Self {
        self.state.creator = Some(uid);
        self
    }

    pub fn creator(&self) -> Option<OsString> {
        if let Some(uid) = self.state.creator {
            let command = create_syscall();
            let user_name = command.get_pwuid(uid);
            if let Some(user_name) = user_name {
                return Some((&*user_name).to_owned());
            }
        }

        None
    }

    pub fn update_status(&self, status: ContainerStatus) -> Self {
        let created = match (status, self.state.created) {
            (ContainerStatus::Created, None) => Some(Utc::now()),
            _ => self.state.created,
        };

        let mut new_state = self.state.clone();
        new_state.created = created;
        new_state.status = status;

        Self {
            state: new_state,
            root: self.root.clone(),
        }
    }

    pub fn load(container_root: PathBuf) -> Result<Self> {
        let state = State::load(&container_root)?;
        Ok(Self {
            state,
            root: container_root,
        })
    }

    pub fn bundle(&self) -> String {
        self.state.bundle.clone()
    }
}
