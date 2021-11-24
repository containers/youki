//! Contains functionality of kill container command
use std::{convert::TryInto, path::PathBuf};

use anyhow::Result;
use clap::Parser;

use crate::commands::load_container;
use libcontainer::signal::Signal;

/// Send the specified signal to the container
#[derive(Parser, Debug)]
pub struct Kill {
    #[clap(forbid_empty_values = true, required = true)]
    container_id: String,
    signal: String,
}

pub fn kill(args: Kill, root_path: PathBuf) -> Result<()> {
    let mut container = load_container(root_path, &args.container_id)?;
    let signal: Signal = args.signal.as_str().try_into()?;
    container.kill(signal)
}
