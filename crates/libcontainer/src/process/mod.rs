//! Provides a thin wrapper around fork syscall,
//! with enums and functions specific to youki implemented

use crate::syscall::SyscallError;

pub mod args;
pub mod channel;
pub mod container_init_process;
pub mod container_intermediate_process;
pub mod container_main_process;
pub mod fork;
pub mod intel_rdt;
pub mod message;

type Result<T> = std::result::Result<T, ProcessError>;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("unknown fatal error")]
    Unknown,
    #[error("failed to clone process using clone3")]
    CloneFailed {
        errno: nix::errno::Errno,
        child_name: String,
    },
    #[error("failed init process")]
    InitProcessFailed,
    #[error("failed intermediate process")]
    IntermediateProcessFailed,
    #[error("io error: {0}")]
    UnixIo(#[from] nix::errno::Errno),
    #[error("failed to add task {pid} to cgroup manager")]
    CgroupAdd {
        pid: nix::unistd::Pid,
        err: anyhow::Error,
    },
    #[error("failed to apply resource limits to cgroup")]
    CgroupApply(anyhow::Error),
    #[error("failed to get proc state")]
    Procfs(#[from] procfs::ProcError),
    #[error("missing linux in spec")]
    NoLinuxSpec,
    #[error("missing process in spec")]
    NoProcessSpec,
    #[error("channel error")]
    ChannelError {
        msg: String,
        source: channel::ChannelError,
    },
    #[error("syscall failed")]
    SyscallFailed(#[from] SyscallError),
    #[error("failed to enter namespace")]
    NamespaceError(#[from] crate::namespaces::NamespaceError),
}
