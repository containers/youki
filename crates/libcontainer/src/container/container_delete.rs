use super::{Container, ContainerStatus};
use crate::config::YoukiConfig;
use crate::hooks;
use anyhow::{bail, Context, Result};
use libcgroups;
use nix::sys::signal;
use std::fs;

impl Container {
    /// Deletes the container
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::create_syscall;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.delete(true)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete(&mut self, force: bool) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;

        log::debug!("container status: {:?}", self.status());

        // Check if container is allowed to be deleted based on container status.
        match self.status() {
            ContainerStatus::Stopped => {}
            ContainerStatus::Created => {
                // Here, we differ from the OCI spec, but matches the same
                // behavior as `runc` and `crun`. The OCI spec does not allow
                // deletion of status `created` without `force` flag. But both
                // `runc` and `crun` allows deleting `created`. Therefore we
                // decided to follow `runc` and `crun`. In the case where
                // container is in `created` status, we first kill the container
                // init process which is waiting on start signal. Since only a
                // single container process (the init process) is created, we do
                // not need to send signals to all in this case.
                self.do_kill(signal::Signal::SIGKILL, false)?;
                self.set_status(ContainerStatus::Stopped).save()?;
            }
            ContainerStatus::Creating | ContainerStatus::Running | ContainerStatus::Paused => {
                // Containers can't be deleted while in these status, unless
                // force flag is set. In the force case, we need to clean up any
                // processes launched by containers.
                if force {
                    self.do_kill(signal::Signal::SIGKILL, true)?;
                    self.set_status(ContainerStatus::Stopped).save()?;
                } else {
                    bail!(
                        "{} could not be deleted because it was {:?}",
                        self.id(),
                        self.status()
                    )
                }
            }
        }

        // Once reached here, the container is verified that it can be deleted.
        debug_assert!(self.status().can_delete());

        if self.root.exists() {
            match YoukiConfig::load(&self.root) {
                Ok(config) => {
                    log::debug!("config: {:?}", config);

                    // remove the cgroup created for the container
                    // check https://man7.org/linux/man-pages/man7/cgroups.7.html
                    // creating and removing cgroups section for more information on cgroups
                    let use_systemd = self
                        .systemd()
                        .context("container state does not contain cgroup manager")?;
                    let cmanager = libcgroups::common::create_cgroup_manager(
                        &config.cgroup_path,
                        use_systemd,
                        self.id(),
                    )
                    .context("failed to create cgroup manager")?;
                    cmanager.remove().with_context(|| {
                        format!("failed to remove cgroup {}", config.cgroup_path.display())
                    })?;

                    if let Some(hooks) = config.hooks.as_ref() {
                        hooks::run_hooks(hooks.poststop().as_ref(), Some(self))
                            .with_context(|| "failed to run post stop hooks")?;
                    }
                }
                Err(err) => {
                    // There is a brief window where the container state is
                    // created, but the container config is not yet generated
                    // from the OCI spec. In this case, we assume as if we
                    // successfully deleted the config and moving on.
                    log::warn!("skipping loading youki config due to: {err:?}, continue to delete");
                }
            }

            // remove the directory storing container state
            log::debug!("remove dir {:?}", self.root);
            fs::remove_dir_all(&self.root).with_context(|| {
                format!("failed to remove container dir {}", self.root.display())
            })?;
        }

        Ok(())
    }
}
