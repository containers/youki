use super::{Container, ContainerStatus};
use anyhow::{bail, Result};
use nix::sys::signal::{self, Signal};

impl Container {
    pub fn kill(&self, signal: Signal) -> Result<()> {
        if self.can_kill() {
            log::debug!("kill signal {} to {}", signal, self.pid().unwrap());
            signal::kill(self.pid().unwrap(), signal)?;
            self.update_status(ContainerStatus::Stopped).save()?;
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
