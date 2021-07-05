use anyhow::{bail, Result};
use oci_spec::Spec;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{notify_socket::NotifyListener, rootless::detect_rootless, tty};

use super::{builder::ContainerBuilder, builder_impl::ContainerBuilderImpl};

/// Builder that can be used to configure the properties of a process
/// that will join an existing container sandbox
pub struct TenantContainerBuilder {
    base: ContainerBuilder,
    env: HashMap<String, String>,
    cwd: Option<PathBuf>,
    command: Vec<String>,
}

impl TenantContainerBuilder {
    /// Generates the base configuration for a process that will join
    /// an existing container sandbox from which configuration methods
    /// can be chained
    pub(super) fn new(builder: ContainerBuilder) -> Self {
        Self {
            base: builder,
            env: HashMap::new(),
            cwd: None,
            command: vec!["sh".to_owned()],
        }
    }

    /// Sets environment variables for the container
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Sets the working directory of the container
    pub fn with_cwd<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.cwd = Some(path.into());
        self
    }

    /// Sets the command the container will be started with
    pub fn with_container_command(mut self, command: Vec<String>) -> Self {
        self.command = command;
        self
    }

    /// Joins an existing container
    pub fn build(self) -> Result<()> {
        let container_dir = self.lookup_container_dir()?;
        let spec = self.load_init_spec(&container_dir)?;

        let notify_socket: NotifyListener = NotifyListener::new(&container_dir)?;
        // convert path of root file system of the container to absolute path
        let rootfs = fs::canonicalize(&spec.root.path)?;

        // if socket file path is given in commandline options,
        // get file descriptors of console socket
        let csocketfd = if let Some(console_socket) = &self.base.console_socket {
            Some(tty::setup_console_socket(&container_dir, console_socket)?)
        } else {
            None
        };

        let rootless = detect_rootless(&spec)?;

        let mut builder_impl = ContainerBuilderImpl {
            init: false,
            syscall: self.base.syscall,
            container_id: self.base.container_id,
            root_path: self.base.root_path,
            pid_file: self.base.pid_file,
            console_socket: csocketfd,
            use_systemd: false,
            container_dir,
            spec,
            rootfs,
            rootless,
            notify_socket,
            container: None,
        };

        builder_impl.create()?;
        Ok(())
    }

    fn lookup_container_dir(&self) -> Result<PathBuf> {
        let container_dir = self.base.root_path.join(&self.base.container_id);
        if !container_dir.exists() {
            bail!("container {} does not exist", self.base.container_id);
        }

        Ok(container_dir)
    }

    fn load_init_spec(&self, container_dir: &Path) -> Result<Spec> {
        let spec_path = container_dir.join("config.json");

        let spec = oci_spec::Spec::load(spec_path)?;
        Ok(spec)
    }
}
