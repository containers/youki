use crate::error::LibcontainerError;

use super::{Container, ContainerStatus};
use libcgroups::common::{CgroupManager, FreezerState};

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
    pub fn pause(&mut self) -> Result<(), LibcontainerError> {
        self.refresh_status()?;

        if !self.can_pause() {
            tracing::error!(status = ?self.status(), id = ?self.id(), "cannot pause container");
            return Err(LibcontainerError::IncorrectStatus);
        }

        let cgroups_path = self.spec()?.cgroup_path;
        let use_systemd = self.systemd();
        let cmanager =
            libcgroups::common::create_cgroup_manager(cgroups_path, use_systemd, self.id())?;
        cmanager.freeze(FreezerState::Frozen)?;

        tracing::debug!("saving paused status");
        self.set_status(ContainerStatus::Paused).save()?;

        tracing::debug!("container {} paused", self.id());
        Ok(())
    }
}
