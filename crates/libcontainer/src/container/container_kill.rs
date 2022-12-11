use super::{Container, ContainerStatus};
use crate::signal::Signal;
use anyhow::{bail, Context, Result};
use libcgroups::common::{create_cgroup_manager, get_cgroup_setup};
use nix::sys::signal::{self};

impl Container {
    /// Sends the specified signal to the container init process
    ///
    /// # Example
    ///
    /// ```no_run
    /// use libcontainer::container::builder::ContainerBuilder;
    /// use libcontainer::syscall::syscall::create_syscall;
    /// use libcontainer::workload::default::DefaultExecutor;
    /// use nix::sys::signal::Signal;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new(
    ///     "74f1a4cb3801".to_owned(),
    ///     create_syscall().as_ref(),
    ///     vec![Box::new(DefaultExecutor::default())],
    /// )
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.kill(Signal::SIGKILL, false)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn kill<S: Into<Signal>>(&mut self, signal: S, all: bool) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;
        if self.can_kill() {
            self.do_kill(signal, all)?;
        } else {
            // just like runc, allow kill --all even if the container is stopped
            if all && self.status() == ContainerStatus::Stopped {
                self.do_kill(signal, all)?;
            } else {
                bail!(
                    "{} could not be killed because it was {:?}",
                    self.id(),
                    self.status()
                )
            }
        }
        self.set_status(ContainerStatus::Stopped).save()?;
        Ok(())
    }

    pub(crate) fn do_kill<S: Into<Signal>>(&self, signal: S, all: bool) -> Result<()> {
        if all {
            self.kill_all_processes(signal)
        } else {
            self.kill_one_process(signal)
        }
    }

    fn kill_one_process<S: Into<Signal>>(&self, signal: S) -> Result<()> {
        let signal = signal.into().into_raw();
        let pid = self.pid().unwrap();

        log::debug!("kill signal {} to {}", signal, pid);
        let res = signal::kill(pid, signal);

        match res {
            Err(nix::errno::Errno::ESRCH) => {
                /* the process does not exist, which is what we want */
            }
            _ => res?,
        }

        // For cgroup V1, a frozon process cannot respond to signals,
        // so we need to thaw it. Only thaw the cgroup for SIGKILL.
        if self.status() == ContainerStatus::Paused && signal == signal::Signal::SIGKILL {
            match get_cgroup_setup()? {
                libcgroups::common::CgroupSetup::Legacy
                | libcgroups::common::CgroupSetup::Hybrid => {
                    let cgroups_path = self.spec()?.cgroup_path;
                    let use_systemd = self
                        .systemd()
                        .context("container state does not contain cgroup manager")?;
                    let cmanger = create_cgroup_manager(&cgroups_path, use_systemd, self.id())?;
                    cmanger.freeze(libcgroups::common::FreezerState::Thawed)?;
                }
                libcgroups::common::CgroupSetup::Unified => {}
            }
        }
        Ok(())
    }

    fn kill_all_processes<S: Into<Signal>>(&self, signal: S) -> Result<()> {
        let signal = signal.into().into_raw();
        let cgroups_path = self.spec()?.cgroup_path;
        let use_systemd = self
            .systemd()
            .context("container state does not contain cgroup manager")?;
        let cmanger = create_cgroup_manager(&cgroups_path, use_systemd, self.id())?;
        let ret = cmanger.freeze(libcgroups::common::FreezerState::Frozen);
        if ret.is_err() {
            log::warn!(
                "failed to freeze container {}, error: {}",
                self.id(),
                ret.unwrap_err()
            );
        }
        let pids = cmanger.get_all_pids()?;
        pids.iter().try_for_each(|&pid| {
            log::debug!("kill signal {} to {}", signal, pid);
            let res = signal::kill(pid, signal);
            match res {
                Err(nix::errno::Errno::ESRCH) => {
                    /* the process does not exist, which is what we want */
                    Ok(())
                }
                _ => res,
            }
        })?;
        let ret = cmanger.freeze(libcgroups::common::FreezerState::Thawed);
        if ret.is_err() {
            log::warn!(
                "failed to thaw container {}, error: {}",
                self.id(),
                ret.unwrap_err()
            );
        }
        Ok(())
    }
}
