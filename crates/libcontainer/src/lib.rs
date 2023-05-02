pub mod apparmor;
pub mod capabilities;
pub mod config;
pub mod container;
pub mod hooks;
pub mod namespaces;
pub mod notify_socket;
pub mod process;
pub mod rootfs;
pub mod rootless;
#[cfg(feature = "libseccomp")]
pub mod seccomp;
pub mod signal;
pub mod syscall;
pub mod tty;
pub mod utils;
pub mod workload;
use std::{any::Any, result::Result as StdResult};
use thiserror::Error as ThisError;

pub type Result<T> = StdResult<T, LibcontainerError>;

#[derive(ThisError, Debug)]
pub enum LibcontainerError {
    #[error("unknown fatal error {0}")]
    UnknownLegacy(#[from] anyhow::Error),
    #[error("unknown fatal error {0}")]
    UnknownWithMsg(String),
    #[error("unknown fatal error")]
    Unknown,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("io error: {0}")]
    UnixIo(#[from] nix::errno::Errno),
    #[error("failed to clone process using clone3")]
    CloneFailed{
        errno: nix::errno::Errno,
        child_name: String,
    },
    #[error("failed to add task {pid} to cgroup manager")]
    CgroupAdd {
        pid: nix::unistd::Pid,
        err: anyhow::Error,
    },
    #[error("failed to apply resource limits to cgroup")]
    CgroupApply(anyhow::Error),
    #[error("failed to get proc state")]
    Procfs(#[from] procfs::ProcError),
}

impl From<Box<dyn Any + Send>> for LibcontainerError {
    fn from(e: Box<dyn Any + Send>) -> Self {
        if e.downcast_ref::<LibcontainerError>().is_none() {
            match e.downcast_ref::<&'static str>() {
                Some(s) => LibcontainerError::UnknownWithMsg(s.to_string()),
                None => match e.downcast_ref::<String>() {
                    Some(s) => LibcontainerError::UnknownWithMsg(s.into()),
                    None => LibcontainerError::Unknown,
                },
            }
        } else {
            match e.downcast::<LibcontainerError>() {
                Ok(ae) => *ae,
                Err(_) => LibcontainerError::Unknown,
            }
        }
    }
}
