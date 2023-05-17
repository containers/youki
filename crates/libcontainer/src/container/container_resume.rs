use crate::error::LibcontainerError;

use super::{Container, ContainerStatus};

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
    pub fn resume(&mut self) -> Result<(), LibcontainerError> {
        self.refresh_status()?;
        // check if container can be resumed :
        // for example, a running process cannot be resumed
        if !self.can_resume() {
            tracing::error!(status = ?self.status(), id = ?self.id(), "cannot resume container");
            return Err(LibcontainerError::IncorrectStatus);
        }

        let cgroups_path = self.spec()?.cgroup_path;
        let use_systemd = self.systemd();
        let cmanager =
            libcgroups::common::create_cgroup_manager(cgroups_path, use_systemd, self.id())?;
        // resume the frozen container
        cmanager.freeze(FreezerState::Thawed)?;

        tracing::debug!("saving running status");
        self.set_status(ContainerStatus::Running).save()?;

        tracing::debug!("container {} resumed", self.id());
        Ok(())
    }
}
