use anyhow::{bail, Context, Result};
use nix::unistd;
use oci_spec::Spec;
use rootless::detect_rootless;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{notify_socket::NotifyListener, rootless, tty, utils};

use super::{
    builder::ContainerBuilder, builder_impl::ContainerBuilderImpl, Container, ContainerStatus,
};

// Builder that can be used to configure the properties of a new container
pub struct InitContainerBuilder {
    base: ContainerBuilder,
    bundle: PathBuf,
    use_systemd: bool,
}

impl InitContainerBuilder {
    /// Generates the base configuration for a new container from which
    /// configuration methods can be chained
    pub(super) fn new(builder: ContainerBuilder, bundle: PathBuf) -> Self {
        Self {
            base: builder,
            bundle,
            use_systemd: true,
        }
    }

    /// Sets if systemd should be used for managing cgroups
    pub fn with_systemd(mut self, should_use: bool) -> Self {
        self.use_systemd = should_use;
        self
    }

    /// Creates a new container
    pub fn build(self) -> Result<()> {
        let container_dir = self.create_container_dir()?;
        let spec = self.load_and_safeguard_spec(&container_dir)?;

        unistd::chdir(&*container_dir)?;
        let container_state = self.create_container_state(&container_dir)?;

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
            init: true,
            syscall: self.base.syscall,
            container_id: self.base.container_id,
            root_path: self.base.root_path,
            pid_file: self.base.pid_file,
            console_socket: csocketfd,
            use_systemd: self.use_systemd,
            container_dir,
            spec,
            rootfs,
            rootless,
            notify_socket,
            container: Some(container_state),
        };

        builder_impl.create()?;
        Ok(())
    }

    fn create_container_dir(&self) -> Result<PathBuf> {
        let container_dir = self.base.root_path.join(&self.base.container_id);
        log::debug!("container directory will be {:?}", container_dir);

        if container_dir.exists() {
            bail!("container {} already exists", self.base.container_id);
        }

        utils::create_dir_all(&container_dir)?;
        Ok(container_dir)
    }

    fn load_and_safeguard_spec(&self, container_dir: &Path) -> Result<Spec> {
        let source_spec_path = self.bundle.join("config.json");
        let target_spec_path = container_dir.join("config.json");
        fs::copy(&source_spec_path, &target_spec_path).with_context(|| {
            format!(
                "failed to copy {:?} to {:?}",
                source_spec_path, target_spec_path
            )
        })?;

        let mut spec = oci_spec::Spec::load(&target_spec_path)?;
        unistd::chdir(&self.bundle)?;
        spec.canonicalize_rootfs()?;
        Ok(spec)
    }

    fn create_container_state(&self, container_dir: &Path) -> Result<Container> {
        let container = Container::new(
            &self.base.container_id,
            ContainerStatus::Creating,
            None,
            self.bundle.as_path().to_str().unwrap(),
            &container_dir,
        )?;
        container.save()?;
        Ok(container)
    }
}
