//! Contains a wrapper of syscalls for unit tests
//! This provides a uniform interface for rest of Youki
//! to call syscalls required for container management

pub mod linux;
#[allow(clippy::module_inception)]
pub mod syscall;
pub mod test;

pub use syscall::Syscall;
#[derive(Debug, thiserror::Error)]
pub enum SyscallError {
    #[error("unexpected mount attr option: {0}")]
    UnexpectedMountAttrOption(String),
    #[error("set keep capabilities to {value}")]
    PrctlSetKeepCapabilites {
        #[source]
        errno: nix::errno::Errno,
        value: bool,
    },
    #[error("set hostname to {hostname}")]
    SetHostname {
        #[source]
        errno: nix::errno::Errno,
        hostname: String,
    },
    #[error("set domainname to {domainname}")]
    SetDomainname {
        #[source]
        errno: nix::errno::Errno,
        domainname: String,
    },
    #[error("{0} is not an actual procfs")]
    NotProcfs(String),
    #[error("failed to get open fds")]
    GetOpenFds(#[source] std::io::Error),
    #[error("failed to pivot root")]
    PivotRoot { source: nix::errno::Errno },
    #[error("failed to setns: {0}")]
    SetNamespace(nix::errno::Errno),
    #[error("failed to set real gid to {gid}")]
    SetRealGid {
        #[source]
        errno: nix::errno::Errno,
        gid: nix::unistd::Gid,
    },
    #[error("failed to set real uid to {uid}: {errno}")]
    SetRealUid {
        #[source]
        errno: nix::errno::Errno,
        uid: nix::unistd::Uid,
    },
    #[error("failed to unshare: {0}")]
    Unshare(nix::errno::Errno),
    #[error("failed to set capabilities: {0}")]
    SetCaps(#[from] caps::errors::CapsError),
    #[error("failed to set rlimit {rlimit:?}")]
    SetRlimit {
        #[source]
        errno: nix::errno::Errno,
        rlimit: oci_spec::runtime::LinuxRlimitType,
    },
    #[error("failed to chroot: {source}")]
    Chroot { source: nix::errno::Errno },
    #[error("mount failed")]
    Mount { source: nix::errno::Errno },
    #[error("symlink failed")]
    Symlink { source: std::io::Error },
    #[error("mknod failed")]
    Mknod { source: nix::errno::Errno },
    #[error("chown failed")]
    Chown { source: nix::errno::Errno },
    #[error("setgroups failed")]
    SetGroups { source: nix::errno::Errno },
    #[error("close range failed")]
    CloseRange {
        preserve_fds: i32,
        #[source]
        errno: syscalls::Errno,
    },
    #[error("invalid filename: {0:?}")]
    InvalidFilename(std::path::PathBuf),
    #[error("mount_setattr failed")]
    MountSetattr { source: syscalls::Errno },
}

type Result<T> = std::result::Result<T, SyscallError>;
