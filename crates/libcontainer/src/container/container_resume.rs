use super::{Container, ContainerStatus};

use anyhow::{bail, Context, Result};
use libcgroups::common::{CgroupManager, FreezerState};

impl Container {
    /// Resumes all processes within the container
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
    /// container.resume()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn resume(&mut self) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;
        // check if container can be resumed :
        // for example, a running process cannot be resumed
        if !self.can_resume() {
            bail!(
                "{} could not be resumed because it was {:?}",
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
        // resume the frozen container
        cmanager.freeze(FreezerState::Thawed)?;

        log::debug!("saving running status");
        self.set_status(ContainerStatus::Running).save()?;

        log::debug!("container {} resumed", self.id());
        Ok(())
    }
}
