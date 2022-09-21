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
    /// use nix::sys::signal::Signal;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
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
            self.do_kill(signal, all, false)?;
            self.set_status(ContainerStatus::Stopped).save()?;
            std::process::exit(0)
        } else {
            bail!(
                "{} could not be killed because it was {:?}",
                self.id(),
                self.status()
            )
        }
    }

    pub(crate) fn do_kill<S: Into<Signal>>(
        &mut self,
        signal: S,
        all: bool,
        force: bool,
    ) -> Result<()> {
        let signal = signal.into().into_raw();
        let mut manger: Option<Box<dyn libcgroups::common::CgroupManager>> = None;
        let pids = if all {
            let cgroups_path = self.spec()?.cgroup_path;
            let use_systemd = self
                .systemd()
                .context("container state does not contain cgroup manager")?;
            let cmanger = create_cgroup_manager(&cgroups_path, use_systemd, self.id())?;
            let all_pids = cmanger.get_all_pids()?;
            manger = Some(cmanger);
            all_pids
        } else {
            vec![self
                .pid()
                .context("failed to get the pid of the container")?]
        };
        if self.status() == ContainerStatus::Paused {
            // In cgroup V1, paused processes cannot respond to signals,
            // so if we want to kill a process forcely, we need to thaw it.
            match get_cgroup_setup()? {
                libcgroups::common::CgroupSetup::Legacy
                | libcgroups::common::CgroupSetup::Hybrid => {
                    if !force {
                        bail!("in cgroup v1, frozon container can't be killed until it is thawed")
                    }
                    let cmanger = if let Some(cmanger) = manger {
                        cmanger
                    } else {
                        let cgroups_path = self.spec()?.cgroup_path;
                        let use_systemd = self
                            .systemd()
                            .context("container state does not contain cgroup manager")?;
                        create_cgroup_manager(&cgroups_path, use_systemd, self.id())?
                    };
                    cmanger.freeze(libcgroups::common::FreezerState::Thawed)?;
                }
                libcgroups::common::CgroupSetup::Unified => {}
            }
        }
        pids.into_iter().try_for_each(|pid| {
            log::debug!("kill signal {} to {}", signal, pid);
            signal::kill(pid, signal)
        })?;
        Ok(())
    }
}
