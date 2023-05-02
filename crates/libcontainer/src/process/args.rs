use libcgroups::common::AnyManager;
use oci_spec::runtime::Spec;
use std::os::unix::prelude::RawFd;
use std::path::PathBuf;

use crate::rootless::Rootless;
use crate::workload::ExecutorManager;
use crate::{container::Container, notify_socket::NotifyListener, syscall::Syscall};

#[derive(Debug, Copy, Clone)]
pub enum ContainerType {
    InitContainer,
    TenantContainer { exec_notify_fd: RawFd },
}

pub struct ContainerArgs<'a> {
    /// Indicates if an init or a tenant container should be created
    pub container_type: ContainerType,
    /// Interface to operating system primitives
    pub syscall: &'a dyn Syscall,
    /// OCI complient runtime spec
    pub spec: &'a Spec,
    /// Root filesystem of the container
    pub rootfs: &'a PathBuf,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// The Unix Domain Socket to communicate container start
    pub notify_socket: NotifyListener,
    /// File descriptors preserved/passed to the container init process.
    pub preserve_fds: i32,
    /// Container state
    pub container: &'a Option<Container>,
    /// Options for rootless containers
    pub rootless: &'a Option<Rootless<'a>>,
    /// Cgroup Manager
    pub cgroup_manager: AnyManager,
    /// If the container is to be run in detached mode
    pub detached: bool,
    /// Manage the functions that actually run on the container
    pub executor_manager: &'a ExecutorManager,
}
