use libcgroups::common::CgroupConfig;
use oci_spec::runtime::Spec;
use std::os::unix::prelude::RawFd;
use std::path::PathBuf;

use crate::container::Container;
use crate::rootless::Rootless;
use crate::syscall::syscall::SyscallType;
use crate::workload::ExecutorManager;

#[derive(Debug, Copy, Clone)]
pub enum ContainerType {
    InitContainer,
    TenantContainer { exec_notify_fd: RawFd },
}

#[derive(Clone)]
pub struct ContainerArgs<'a> {
    /// Indicates if an init or a tenant container should be created
    pub container_type: ContainerType,
    /// Interface to operating system primitives
    pub syscall: SyscallType,
    /// OCI compliant runtime spec
    pub spec: &'a Spec,
    /// Root filesystem of the container
    pub rootfs: &'a PathBuf,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// The Unix Domain Socket to communicate container start
    pub notify_socket_path: PathBuf,
    /// File descriptors preserved/passed to the container init process.
    pub preserve_fds: i32,
    /// Container state
    pub container: &'a Option<Container>,
    /// Options for rootless containers
    pub rootless: &'a Option<Rootless<'a>>,
    /// Cgroup Manager Config
    pub cgroup_config: CgroupConfig,
    /// If the container is to be run in detached mode
    pub detached: bool,
    /// Manage the functions that actually run on the container
    pub executor_manager: &'a ExecutorManager,
}
