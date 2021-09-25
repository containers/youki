use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};
use nix::sys::signal::{self, Signal};

impl Container {
    /// Sends the specified signal to the container init process
    ///
    /// # Example
    ///
    /// ```no_run
    /// use youki::container::builder::ContainerBuilder;
    /// use youki::syscall::syscall::create_syscall;;
    /// use nix::sys::signal::Signal;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut container = ContainerBuilder::new("74f1a4cb3801".to_owned(), create_syscall().as_ref())
    /// .as_init("/var/run/docker/bundle")
    /// .build()?;
    ///
    /// container.kill(Signal::SIGKILL)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn kill(&mut self, signal: Signal) -> Result<()> {
        self.refresh_status()
            .context("failed to refresh container status")?;
        if self.can_kill() {
            log::debug!("kill signal {} to {}", signal, self.pid().unwrap());
            signal::kill(self.pid().unwrap(), signal)?;
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
}
