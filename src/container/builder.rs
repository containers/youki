use crate::command::linux::LinuxSyscall;
use std::path::PathBuf;

use super::{init_builder::InitContainerBuilder, tenant_builder::TenantContainerBuilder};
pub struct ContainerBuilder {
    pub(super) container_id: String,

    pub(super) root_path: PathBuf,

    pub(super) syscall: LinuxSyscall,

    pub(super) pid_file: Option<PathBuf>,

    pub(super) console_socket: Option<PathBuf>,
}

/// Builder that can be used to configure the common properties of
/// either a init or a tenant container
///
/// # Example
///
/// ```no_run
/// use youki::container::builder::ContainerBuilder;
///
/// ContainerBuilder::new("74f1a4cb3801".to_owned())
/// .with_root_path("/run/containers/youki")
/// .with_pid_file("/var/run/docker.pid")
/// .with_console_socket("/var/run/docker/sock.tty")
/// .as_init("/var/run/docker/bundle")
/// .build();
/// ```
impl ContainerBuilder {
    /// Generates the base configuration for a container which can be
    /// transformed into either a init container or a tenant container
    ///
    /// # Example
    ///
    /// ```no_run
    /// use youki::container::builder::ContainerBuilder;
    ///
    /// let builder = ContainerBuilder::new("74f1a4cb3801".to_owned());
    /// ```
    pub fn new(container_id: String) -> Self {
        let root_path = PathBuf::from("/run/youki");

        Self {
            container_id,
            root_path,
            syscall: LinuxSyscall,
            pid_file: None,
            console_socket: None,
        }
    }

    /// Transforms this builder into a tenant builder
    /// # Example
    ///
    /// ```no_run
    /// # use youki::container::builder::ContainerBuilder;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned())
    /// .as_tenant()
    /// .with_container_command(vec!["sleep".to_owned(), "9001".to_owned()])
    /// .build();
    /// ```
    #[allow(clippy::wrong_self_convention)]
    pub fn as_tenant(self) -> TenantContainerBuilder {
        TenantContainerBuilder::new(self)
    }

    /// Transforms this builder into an init builder
    /// # Example
    ///
    /// ```no_run
    /// # use youki::container::builder::ContainerBuilder;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned())
    /// .as_init("/var/run/docker/bundle")
    /// .with_systemd(false)
    /// .build();
    /// ```
    #[allow(clippy::wrong_self_convention)]
    pub fn as_init<P: Into<PathBuf>>(self, bundle: P) -> InitContainerBuilder {
        InitContainerBuilder::new(self, bundle.into())
    }

    /// Sets the root path which will be used to store the container state
    /// # Example
    ///
    /// ```no_run
    /// # use youki::container::builder::ContainerBuilder;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned())
    /// .with_root_path("/run/containers/youki");
    /// ```
    pub fn with_root_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.root_path = path.into();
        self
    }

    /// Sets the pid file which will be used to write the pid of the container
    /// process
    /// # Example
    ///
    /// ```no_run
    /// # use youki::container::builder::ContainerBuilder;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned())
    /// .with_pid_file("/var/run/docker.pid");
    /// ```
    pub fn with_pid_file<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.pid_file = Some(path.into());
        self
    }

    /// Sets the console socket, which will be used to send the file descriptor
    /// of the pseudoterminal
    /// # Example
    ///
    /// ```no_run
    /// # use youki::container::builder::ContainerBuilder;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned())
    /// .with_console_socket("/var/run/docker/sock.tty");
    /// ```
    pub fn with_console_socket<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.console_socket = Some(path.into());
        self
    }
}
