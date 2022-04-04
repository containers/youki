//! Contains functionality of kill container command
use std::{convert::TryInto, path::PathBuf};

use anyhow::Result;

use crate::commands::load_container;
use libcontainer::signal::Signal;
use liboci_cli::Kill;

pub fn kill(args: Kill, root_path: PathBuf) -> Result<()> {
    let mut container = load_container(root_path, &args.container_id)?;
    let signal: Signal = args.signal.as_str().try_into()?;
    container.kill(signal)
}
