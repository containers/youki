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
    #[error("set keep capabilities to {value} returned {errno}")]
    PrctlSetKeepCapabilitesFailed {
        errno: nix::errno::Errno,
        value: bool,
    },
    #[error("set hostname to {hostname} returned {errno}")]
    SetHostnameFailed{
        errno: nix::errno::Errno,
        hostname: String,
    },
    #[error("set domainname to {domainname} returned {errno}")]
    SetDomainnameFailed{
        errno: nix::errno::Errno,
        domainname: String,
    },
    #[error("{0} is not an actual procfs")]
    NotProcfs(String),
    #[error("failed to get open fds: {0}")]
    GetOpenFdsFailed(std::io::Error),
    #[error("failed to pivot root")]
    PivotRootFailed{
        path: String,
        msg: String,
        errno: nix::errno::Errno,
    },
    #[error("failed to setns: {0}")]
    SetNamespaceFailed(nix::errno::Errno),
    #[error("failed to set real gid to {gid}: {errno}")]
    SetRealGidFailed{
        errno: nix::errno::Errno,
        gid: nix::unistd::Gid,
    },
    #[error("failed to set real uid to {uid}: {errno}")]
    SetRealUidFailed {
        errno: nix::errno::Errno,
        uid: nix::unistd::Uid,
    },
    #[error("failed to unshare: {0}")]
    UnshareFailed(nix::errno::Errno),
    #[error("failed to set capabilities: {0}")]
    SetCapsFailed(#[from] caps::errors::CapsError),
    #[error("failed to set rlimit {rlimit:?}: {errno}")]
    SetRlimitFailed {
        errno: nix::errno::Errno,
        rlimit: oci_spec::runtime::LinuxRlimitType,
    },
    #[error("failed to chroot to {path:?}: {errno}")]
    ChrootFailed {
        path: std::path::PathBuf,
        errno: nix::errno::Errno,
    },
    #[error("mount failed")]
    MountFailed {
        mount_source: Option<std::path::PathBuf>,
        mount_target: std::path::PathBuf,
        fstype: Option<String>,
        flags: nix::mount::MsFlags,
        data: Option<String>,
        errno: nix::errno::Errno,
    },
    #[error("symlink failed")]
    SymlinkFailed {
        old_path: std::path::PathBuf,
        new_path: std::path::PathBuf,
        err: std::io::Error,
    },
    #[error("mknod failed")]
    MknodFailed {
        path: std::path::PathBuf,
        kind: nix::sys::stat::SFlag,
        perm: nix::sys::stat::Mode,
        dev: nix::sys::stat::dev_t,
        errno: nix::errno::Errno,
    },
    #[error("chown failed")]
    ChownFailed {
        path: std::path::PathBuf,
        owner: Option<nix::unistd::Uid>,
        group: Option<nix::unistd::Gid>,
        errno: nix::errno::Errno,
    },
    #[error("setgroups failed")]
    SetGroupsFailed {
        groups: Vec<nix::unistd::Gid>,
        errno: nix::errno::Errno,
    },
    #[error("close range failed")]
    CloseRangeFailed {
        preserve_fds: i32,
        errno: syscalls::Errno,
    },
    #[error("invalid filename: {0:?}")]
    InvalidFilename(std::path::PathBuf),
    #[error("mount_setattr failed")]
    MountSetattrFailed {
        pathname: std::path::PathBuf,
        flags: u32,
        errno: syscalls::Errno,
    },
}

type Result<T> = std::result::Result<T, SyscallError>;
