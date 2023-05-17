//! Contains functionality of kill container command
use std::{convert::TryInto, path::PathBuf};

use anyhow::{anyhow, Result};

use crate::commands::load_container;
use libcontainer::{container::ContainerStatus, signal::Signal};
use liboci_cli::Kill;

pub fn kill(args: Kill, root_path: PathBuf) -> Result<()> {
    let mut container = load_container(root_path, &args.container_id)?;
    let signal: Signal = args.signal.as_str().try_into()?;
    match container.kill(signal, args.all) {
        Ok(_) => Ok(()),
        Err(e) => {
            // see https://github.com/containers/youki/issues/1314
            if container.status() == ContainerStatus::Stopped {
                return Err(anyhow!(e).context("container not running"));
            }
            Err(anyhow!(e).context("failed to kill container"))
        }
    }
}
