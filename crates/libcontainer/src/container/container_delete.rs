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
        if self.can_kill() && force {
            self.do_kill(signal::Signal::SIGKILL, true)?;
            self.set_status(ContainerStatus::Stopped).save()?;
        }
        log::debug!("container status: {:?}", self.status());
        if self.can_delete() {
            if self.root.exists() {
                if let Ok(config) = YoukiConfig::load(&self.root) {
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

                // remove the directory storing container state
                log::debug!("remove dir {:?}", self.root);
                fs::remove_dir_all(&self.root).with_context(|| {
                    format!("failed to remove container dir {}", self.root.display())
                })?;
            }
            Ok(())
        } else {
            bail!(
                "{} could not be deleted because it was {:?}",
                self.id(),
                self.status()
            )
        }
    }
}
