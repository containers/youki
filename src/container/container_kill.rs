use super::{Container, ContainerStatus};
use anyhow::{bail, Context, Result};
use nix::sys::signal::{self, Signal};

impl Container {
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
