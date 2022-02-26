use crate::syscall::Syscall;
use anyhow::{Context, Result};
use std::path::PathBuf;

use super::{init_builder::InitContainerBuilder, tenant_builder::TenantContainerBuilder};

pub struct ContainerBuilder<'a> {
    /// Id of the container
    pub(super) container_id: String,
    /// Root directory for container state
    pub(super) root_path: PathBuf,
    /// Interface to operating system primitives
    pub(super) syscall: &'a dyn Syscall,
    /// File which will be used to communicate the pid of the
    /// container process to the higher level runtime
    pub(super) pid_file: Option<PathBuf>,
    /// Socket to communicate the file descriptor of the ptty
    pub(super) console_socket: Option<PathBuf>,
    /// File descriptors to be passed into the container process
    pub(super) preserve_fds: i32,
}

/// Builder that can be used to configure the common properties of
/// either a init or a tenant container
///
/// # Example
///
/// ```no_run
/// use libcontainer::container::builder::ContainerBuilder;
/// use libcontainer::syscall::syscall::create_syscall;
///
/// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
/// .with_root_path("/run/containers/youki").expect("invalid root path")
/// .with_pid_file(Some("/var/run/docker.pid")).expect("invalid pid file")
/// .with_console_socket(Some("/var/run/docker/sock.tty"))
/// .as_init("/var/run/docker/bundle")
/// .build();
/// ```
impl<'a> ContainerBuilder<'a> {
    /// Generates the base configuration for a container which can be
    /// transformed into either a init container or a tenant container
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::create_syscall;
    ///
    /// let builder = ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref());
    /// ```
    pub fn new(container_id: String, syscall: &'a dyn Syscall) -> Self {
        let root_path = PathBuf::from("/run/youki");

        Self {
            container_id,
            root_path,
            syscall,
            pid_file: None,
            console_socket: None,
            preserve_fds: 0,
        }
    }

    /// Transforms this builder into a tenant builder
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::create_syscall;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .as_tenant()
    /// .with_container_args(vec!["sleep".to_owned(), "9001".to_owned()])
    /// .build();
    /// ```
    #[allow(clippy::wrong_self_convention)]
    pub fn as_tenant(self) -> TenantContainerBuilder<'a> {
        TenantContainerBuilder::new(self)
    }

    /// Transforms this builder into an init builder
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::create_syscall;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .as_init("/var/run/docker/bundle")
    /// .with_systemd(false)
    /// .build();
    /// ```
    #[allow(clippy::wrong_self_convention)]
    pub fn as_init<P: Into<PathBuf>>(self, bundle: P) -> InitContainerBuilder<'a> {
        InitContainerBuilder::new(self, bundle.into())
    }

    /// Sets the root path which will be used to store the container state
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::create_syscall;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .with_root_path("/run/containers/youki").expect("invalid root path");
    /// ```
    pub fn with_root_path<P: Into<PathBuf>>(mut self, path: P) -> Result<Self> {
        self.root_path = path
            .into()
            .canonicalize()
            .context("failed to canonicalize root path")?;
        Ok(self)
    }

    /// Sets the pid file which will be used to write the pid of the container
    /// process
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::create_syscall;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .with_pid_file(Some("/var/run/docker.pid")).expect("invalid pid file");
    /// ```
    pub fn with_pid_file<P: Into<PathBuf>>(mut self, path: Option<P>) -> Result<Self> {
        self.pid_file = if let Some(p) = path {
            Some(
                p.into()
                    .canonicalize()
                    .context("failed canonicalize pid file path")?,
            )
        } else {
            None
        };
        Ok(self)
    }

    /// Sets the console socket, which will be used to send the file descriptor
    /// of the pseudoterminal
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::create_syscall;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .with_console_socket(Some("/var/run/docker/sock.tty"));
    /// ```
    pub fn with_console_socket<P: Into<PathBuf>>(mut self, path: Option<P>) -> Self {
        self.console_socket = path.map(|p| p.into());
        self
    }

    /// Sets the number of additional file descriptors which will be passed into
    /// the container process.
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::create_syscall;
    ///
    /// ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .with_preserved_fds(5);
    /// ```
    pub fn with_preserved_fds(mut self, preserved_fds: i32) -> Self {
        self.preserve_fds = preserved_fds;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::container::builder::ContainerBuilder;
    use crate::syscall::syscall::create_syscall;
    use crate::utils::TempDir;
    use anyhow::{Context, Result};
    use std::path::PathBuf;

    #[test]
    fn test_builder_failable_functions() -> Result<()> {
        let root_path_temp_dir = TempDir::new("root_path").context("failed to create temp dir")?;
        let pid_file_temp_dir = TempDir::new("pid_file").context("failed to create temp dir")?;
        let syscall = create_syscall();

        ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall.as_ref())
            .with_root_path(root_path_temp_dir.path())?
            .with_pid_file(Some(pid_file_temp_dir.path()))?
            .with_console_socket(Some("/var/run/docker/sock.tty"))
            .as_init("/var/run/docker/bundle");

        // Accept None pid file.
        ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall.as_ref())
            .with_pid_file::<PathBuf>(None)?;

        let invalid_path_builder =
            ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall.as_ref())
                .with_root_path("/not/existing/path");
        assert!(invalid_path_builder.is_err());

        let invalid_pid_file_builder =
            ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall.as_ref())
                .with_root_path(root_path_temp_dir.path())?
                .with_pid_file(Some("/not/existing/path"));
        assert!(invalid_pid_file_builder.is_err());

        Ok(())
    }
}
