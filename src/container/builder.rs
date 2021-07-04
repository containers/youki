use crate::command::linux::LinuxCommand;
use std::path::PathBuf;

use super::{init_builder::InitContainerBuilder, tenant_builder::TenantContainerBuilder};
pub struct ContainerBuilder {
    pub(super) container_id: String,

    pub(super) root_path: PathBuf,

    pub(super) syscall: LinuxCommand,

    pub(super) pid_file: Option<PathBuf>,

    pub(super) console_socket: Option<PathBuf>,
}

impl ContainerBuilder {
    pub fn new(container_id: String) -> Self {
        let root_path = PathBuf::from("/run/youki");

        Self {
            container_id,
            root_path,
            syscall: LinuxCommand,
            pid_file: None,
            console_socket: None,
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn as_tenant(self) -> TenantContainerBuilder {
        TenantContainerBuilder::new(self)
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn as_init<P: Into<PathBuf>>(self, bundle: P) -> InitContainerBuilder {
        InitContainerBuilder::new(self, bundle.into())
    }

    pub fn with_root_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.root_path = path.into();
        self
    }

    pub fn with_pid_file<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.pid_file = Some(path.into());
        self
    }

    pub fn with_console_socket<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.console_socket = Some(path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use anyhow::Result;

    // required values (must be specified in new...)
    // - create
    //   - id
    //   - bundle
    // - exec
    //   - id
    //
    // use with_... methods to specify
    // optional values
    // - console-socket
    // - pid-file
    //
    // overwritable values
    // - systemd (default true)
    // - root_path (default /run/youki)
    //
    // overwritable values (for exec only?)
    // - env
    // - cwd
    // - container command
    //
    // calculated in build()
    // computed values
    // - rootless
    // - container_dir
    // - spec
    // - notify_socket
    // - container

    // create
    fn test_create_init() -> Result<()> {
        let id = "".to_owned();
        let bundle = PathBuf::from("");
        let pid_file = PathBuf::from("");
        let console_socket = PathBuf::from("");
        let root_path = PathBuf::from("");

        let container = ContainerBuilder::new(id)
            .with_pid_file(pid_file) // optional
            .with_console_socket(console_socket) //optional
            .with_root_path(root_path) // overwrite default
            .as_init(bundle)
            .with_systemd(false)
            .build()?;

        Ok(())
    }

    // exec
    fn test_create_tenant() -> Result<()> {
        let id = "".to_owned();
        let pid_file = PathBuf::from("");
        let console_socket = PathBuf::from("");
        let cwd = PathBuf::from("");
        let env = HashMap::new();

        let container = ContainerBuilder::new(id)
            .with_pid_file(pid_file)
            .with_console_socket(console_socket)
            .as_tenant()
            .with_cwd(cwd)
            .with_env(env)
            .with_container_command(vec!["sleep".to_owned(), "9001".to_owned()])
            .build()?;

        Ok(())
    }
}
