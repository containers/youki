//! Contains Functionality of `features` container command
use anyhow::Result;
use liboci_cli::Features;

/// lists all existing containers
pub fn features(_: Features) -> Result<()> {
    Ok(())
}
