use oci_spec::runtime::Spec;
use std::os::unix::prelude::RawFd;
use std::path::PathBuf;

use crate::rootless::Rootless;
use crate::{container::Container, notify_socket::NotifyListener, syscall::Syscall};

pub struct ContainerArgs<'a> {
    /// Flag indicating if an init or a tenant container should be created
    pub init: bool,
    /// Interface to operating system primitives
    pub syscall: &'a dyn Syscall,
    /// OCI complient runtime spec
    pub spec: Spec,
    /// Root filesystem of the container
    pub rootfs: PathBuf,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// The Unix Domain Socket to communicate container start
    pub notify_socket: NotifyListener,
    /// File descriptos preserved/passed to the container init process.
    pub preserve_fds: i32,
    /// Container state
    pub container: Option<Container>,
    /// Options for rootless containers
    pub rootless: Option<Rootless<'a>>,
}
