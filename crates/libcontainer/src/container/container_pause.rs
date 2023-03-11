use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};
use libcgroups::common::FreezerState;

impl Container {
    /// Suspends all processes within the container
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::create_syscall;
    /// use libcontainer::workload::default::DefaultExecutor;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     create_syscall().as_ref(),
    /// )
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.pause()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn pause(&mut self) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;

        if !self.can_pause() {
            bail!(
                "{} could not be paused because it was {:?}",
                self.id(),
                self.status()
            );
        }

        let cgroups_path = self.spec()?.cgroup_path;
        let use_systemd = self
            .systemd()
            .context("container state does not contain cgroup manager")?;
        let cmanager =
            libcgroups::common::create_cgroup_manager(cgroups_path, use_systemd, self.id())?;
        cmanager.freeze(FreezerState::Frozen)?;

        log::debug!("saving paused status");
        self.set_status(ContainerStatus::Paused).save()?;

        log::debug!("container {} paused", self.id());
        Ok(())
    }
}
