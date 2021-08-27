use anyhow::Result;
use clap::Clap;
use oci_spec::Spec;
use serde_json::to_writer_pretty;
use std::fs::File;

/// Command generates a config.json
#[derive(Clap, Debug)]
pub struct SpecJson {
    /// Generate a configuration for a rootless container
    #[clap(long)]
    pub rootless: bool,
}

/// spec Cli command
impl SpecJson {
    pub fn exec(&self) -> Result<()> {
        // get default values for Spec
        let mut default_json: Spec = Default::default();
        if self.rootless {
            default_json.set_for_rootless()?;
        }
        // write data to config.json
        to_writer_pretty(&File::create("config.json")?, &default_json)?;
        Ok(())
    }
}
