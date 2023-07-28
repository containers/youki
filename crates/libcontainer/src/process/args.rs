use libcgroups::common::CgroupConfig;
use oci_spec::runtime::Spec;
use std::os::unix::prelude::RawFd;
use std::path::PathBuf;
use std::rc::Rc;

use crate::container::Container;
use crate::notify_socket::NotifyListener;
use crate::rootless::Rootless;
use crate::syscall::syscall::SyscallType;
use crate::workload::Executor;
#[derive(Debug, Copy, Clone)]
pub enum ContainerType {
    InitContainer,
    TenantContainer { exec_notify_fd: RawFd },
}

#[derive(Clone)]
pub struct ContainerArgs {
    /// Indicates if an init or a tenant container should be created
    pub container_type: ContainerType,
    /// Interface to operating system primitives
    pub syscall: SyscallType,
    /// OCI compliant runtime spec
    pub spec: Rc<Spec>,
    /// Root filesystem of the container
    pub rootfs: PathBuf,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// The Unix Domain Socket to communicate container start
    pub notify_listener: NotifyListener,
    /// File descriptors preserved/passed to the container init process.
    pub preserve_fds: i32,
    /// Container state
    pub container: Option<Container>,
    /// Options for rootless containers
    pub rootless: Option<Rootless>,
    /// Cgroup Manager Config
    pub cgroup_config: CgroupConfig,
    /// If the container is to be run in detached mode
    pub detached: bool,
    /// Manage the functions that actually run on the container
    pub executor: Executor,
}
