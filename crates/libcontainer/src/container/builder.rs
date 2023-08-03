use crate::error::{ErrInvalidID, LibcontainerError};
use crate::syscall::syscall::SyscallType;
use crate::utils::PathBufExt;
use crate::workload::{self, Executor};
use std::path::PathBuf;

use super::{init_builder::InitContainerBuilder, tenant_builder::TenantContainerBuilder};

pub struct ContainerBuilder {
    /// Id of the container
    pub(super) container_id: String,
    /// Root directory for container state
    pub(super) root_path: PathBuf,
    /// Interface to operating system primitives
    pub(super) syscall: SyscallType,
    /// File which will be used to communicate the pid of the
    /// container process to the higher level runtime
    pub(super) pid_file: Option<PathBuf>,
    /// Socket to communicate the file descriptor of the ptty
    pub(super) console_socket: Option<PathBuf>,
    /// File descriptors to be passed into the container process
    pub(super) preserve_fds: i32,
    /// The function that actually runs on the container init process. Default
    /// is to execute the specified command in the oci spec.
    pub(super) executor: Box<dyn Executor>,
}

/// Builder that can be used to configure the common properties of
/// either a init or a tenant container
///
/// # Example
///
/// ```no_run
/// use libcontainer::container::builder::ContainerBuilder;
/// use libcontainer::syscall::syscall::SyscallType;
///
/// ContainerBuilder::new(
///     "74f1a4cb3801".to_owned(),
///     SyscallType::default(),
/// )
/// .with_root_path("/run/containers/youki").expect("invalid root path")
/// .with_pid_file(Some("/var/run/docker.pid")).expect("invalid pid file")
/// .with_console_socket(Some("/var/run/docker/sock.tty"))
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
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::SyscallType;
    ///
    /// let builder = ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// );
    /// ```
    pub fn new(container_id: String, syscall: SyscallType) -> Self {
        let root_path = PathBuf::from("/run/youki");
        Self {
            container_id,
            root_path,
            syscall,
            pid_file: None,
            console_socket: None,
            preserve_fds: 0,
            executor: workload::default::get_executor(),
        }
    }

    /// validate_id checks if the supplied container ID is valid, returning
    /// the ErrInvalidID in case it is not.
    ///
    /// The format of valid ID was never formally defined, instead the code
    /// was modified to allow or disallow specific characters.
    ///
    /// Currently, a valid ID is a non-empty string consisting only of
    /// the following characters:
    /// - uppercase (A-Z) and lowercase (a-z) Latin letters;
    /// - digits (0-9);
    /// - underscore (_);
    /// - plus sign (+);
    /// - minus sign (-);
    /// - period (.).
    ///
    /// In addition, IDs that can't be used to represent a file name
    /// (such as . or ..) are rejected.
    pub fn validate_id(self) -> Result<Self, LibcontainerError> {
        let container_id = self.container_id.clone();
        if container_id.is_empty() {
            Err(ErrInvalidID::Empty)?;
        }

        if container_id == "." || container_id == ".." {
            Err(ErrInvalidID::FileName)?;
        }

        for c in container_id.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '+' | '-' | '.' => (),
                _ => Err(ErrInvalidID::InvalidChars(c))?,
            }
        }
        Ok(self)
    }

    /// Transforms this builder into a tenant builder
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::SyscallType;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
    /// .as_tenant()
    /// .with_container_args(vec!["sleep".to_owned(), "9001".to_owned()])
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
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::SyscallType;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
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
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::SyscallType;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
    /// .with_root_path("/run/containers/youki").expect("invalid root path");
    /// ```
    pub fn with_root_path<P: Into<PathBuf>>(mut self, path: P) -> Result<Self, LibcontainerError> {
        let path = path.into();
        self.root_path = path.canonicalize_safely().map_err(|err| {
            tracing::error!(?path, ?err, "failed to canonicalize root path");
            LibcontainerError::InvalidInput(format!("invalid root path {path:?}: {err:?}"))
        })?;

        Ok(self)
    }

    /// Sets the pid file which will be used to write the pid of the container
    /// process
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::SyscallType;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
    /// .with_pid_file(Some("/var/run/docker.pid")).expect("invalid pid file");
    /// ```
    pub fn with_pid_file<P: Into<PathBuf>>(
        mut self,
        path: Option<P>,
    ) -> Result<Self, LibcontainerError> {
        self.pid_file = match path.map(|p| p.into()) {
            Some(path) => Some(path.canonicalize_safely().map_err(|err| {
                tracing::error!(?path, ?err, "failed to canonicalize pid file");
                LibcontainerError::InvalidInput(format!("invalid pid file path {path:?}: {err:?}"))
            })?),
            None => None,
        };

        Ok(self)
    }

    /// Sets the console socket, which will be used to send the file descriptor
    /// of the pseudoterminal
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::SyscallType;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
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
    /// # use libcontainer::syscall::syscall::SyscallType;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
    /// .with_preserved_fds(5);
    /// ```
    pub fn with_preserved_fds(mut self, preserved_fds: i32) -> Self {
        self.preserve_fds = preserved_fds;
        self
    }
    /// Sets the number of additional file descriptors which will be passed into
    /// the container process.
    /// # Example
    ///
    /// ```no_run
    /// # use libcontainer::container::builder::ContainerBuilder;
    /// # use libcontainer::syscall::syscall::SyscallType;
    /// # use libcontainer::workload::default::DefaultExecutor;
    ///
    /// ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
    /// .with_executor(DefaultExecutor{});
    /// ```
    pub fn with_executor(mut self, executor: impl Executor + 'static) -> Self {
        self.executor = Box::new(executor);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{container::builder::ContainerBuilder, syscall::syscall::SyscallType};
    use anyhow::{Context, Result};
    use std::path::PathBuf;

    #[test]
    fn test_failable_functions() -> Result<()> {
        let root_path_temp_dir = tempfile::tempdir().context("failed to create temp dir")?;
        let pid_file_temp_dir = tempfile::tempdir().context("failed to create temp dir")?;
        let syscall = SyscallType::default();

        ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall)
            .with_root_path(root_path_temp_dir.path())?
            .with_pid_file(Some(pid_file_temp_dir.path().join("fake.pid")))?
            .with_console_socket(Some("/var/run/docker/sock.tty"))
            .as_init("/var/run/docker/bundle");

        // accept None pid file.
        ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall).with_pid_file::<PathBuf>(None)?;

        // accept absolute root path which does not exist
        let abs_root_path = PathBuf::from("/not/existing/path");
        let path_builder = ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall)
            .with_root_path(&abs_root_path)
            .context("build container")?;
        assert_eq!(path_builder.root_path, abs_root_path);

        // accept relative root path which does not exist
        let cwd = std::env::current_dir().context("get current dir")?;
        let path_builder = ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall)
            .with_root_path("./not/existing/path")
            .context("build container")?;
        assert_eq!(path_builder.root_path, cwd.join("not/existing/path"));

        // accept absolute pid path which does not exist
        let abs_pid_path = PathBuf::from("/not/existing/path");
        let path_builder = ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall)
            .with_pid_file(Some(&abs_pid_path))
            .context("build container")?;
        assert_eq!(path_builder.pid_file, Some(abs_pid_path));

        // accept relative pid path which does not exist
        let cwd = std::env::current_dir().context("get current dir")?;
        let path_builder = ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall)
            .with_pid_file(Some("./not/existing/path"))
            .context("build container")?;
        assert_eq!(path_builder.pid_file, Some(cwd.join("not/existing/path")));

        Ok(())
    }

    #[test]
    fn test_validate_id() -> Result<()> {
        let syscall = SyscallType::default();
        // validate container_id
        let result = ContainerBuilder::new("$#".to_owned(), syscall).validate_id();
        assert!(result.is_err());

        let result = ContainerBuilder::new(".".to_owned(), syscall).validate_id();
        assert!(result.is_err());

        let result = ContainerBuilder::new("..".to_owned(), syscall).validate_id();
        assert!(result.is_err());

        let result = ContainerBuilder::new("...".to_owned(), syscall).validate_id();
        assert!(result.is_ok());

        let result = ContainerBuilder::new("74f1a4cb3801".to_owned(), syscall).validate_id();
        assert!(result.is_ok());
        Ok(())
    }
}
