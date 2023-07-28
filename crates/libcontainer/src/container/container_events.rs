use std::{thread, time::Duration};

use crate::error::LibcontainerError;

use super::{Container, ContainerStatus};
use libcgroups::common::CgroupManager;

impl Container {
    /// Displays container events
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::SyscallType;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     SyscallType::default(),
    /// )
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.events(5000, false)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn events(&mut self, interval: u32, stats: bool) -> Result<(), LibcontainerError> {
        self.refresh_status()?;
        if !self.state.status.eq(&ContainerStatus::Running) {
            tracing::error!(id = ?self.id(), status = ?self.state.status, "container is not running");
            return Err(LibcontainerError::IncorrectStatus);
        }

        let cgroup_manager =
            libcgroups::common::create_cgroup_manager(libcgroups::common::CgroupConfig {
                cgroup_path: self.spec()?.cgroup_path,
                systemd_cgroup: self.systemd(),
                container_name: self.id().to_string(),
            })?;
        match stats {
            true => {
                let stats = cgroup_manager.stats()?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stats)
                        .map_err(LibcontainerError::OtherSerialization)?
                );
            }
            false => loop {
                let stats = cgroup_manager.stats()?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stats)
                        .map_err(LibcontainerError::OtherSerialization)?
                );
                thread::sleep(Duration::from_secs(interval as u64));
            },
        }

        Ok(())
    }
}
