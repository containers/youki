//! Contains functionality of kill container command
use std::{convert::TryInto, path::PathBuf};

use anyhow::Result;
use clap::Clap;

use crate::{commands::load_container, signal::Signal};

/// Send the specified signal to the container
#[derive(Clap, Debug)]
pub struct Kill {
    #[clap(required = true)]
    container_id: String,
    signal: String,
}

impl Kill {
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let mut container = load_container(root_path, &self.container_id)?;
        let signal: Signal = self.signal.as_str().try_into()?;
        container.kill(signal)
    }
}
