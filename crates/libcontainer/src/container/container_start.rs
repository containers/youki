use crate::{
    config::YoukiConfig,
    hooks,
    notify_socket::{NotifySocket, NOTIFY_FILE},
};

use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};
use nix::unistd;

impl Container {
    /// Starts a previously created container
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::create_syscall;;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.start();
    /// # Ok(())
    /// # }
    /// ```
    pub fn start(&mut self) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;

        if !self.can_start() {
            let err_msg = format!(
                "{} could not be started because it was {:?}",
                self.id(),
                self.status()
            );
            log::error!("{}", err_msg);
            bail!(err_msg);
        }

        let config = YoukiConfig::load(self.root.join("config.json"))
            .with_context(|| format!("failed to load runtime spec for container {}", self.id()))?;
        if let Some(hooks) = config.hooks.as_ref() {
            // While prestart is marked as deprecated in the OCI spec, the docker and integration test still
            // uses it.
            #[allow(deprecated)]
            hooks::run_hooks(hooks.prestart().as_ref(), Some(self))
                .with_context(|| "failed to run pre start hooks")?;
        }

        unistd::chdir(self.root.as_os_str())?;

        let mut notify_socket = NotifySocket::new(&self.root.join(NOTIFY_FILE));
        notify_socket.notify_container_start()?;
        self.set_status(ContainerStatus::Running)
            .save()
            .with_context(|| format!("could not save state for container {}", self.id()))?;

        // Run post start hooks. It runs after the container process is started.
        // It is called in the runtime namespace.
        if let Some(hooks) = config.hooks.as_ref() {
            hooks::run_hooks(hooks.poststart().as_ref(), Some(self))
                .with_context(|| "failed to run post start hooks")?;
        }

        Ok(())
    }
}
