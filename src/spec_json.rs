use anyhow::Result;
use clap::Clap;
use oci_spec::Spec;
use serde_json::to_writer_pretty;
use std::fs::File;

/// Command generates a config.json
#[derive(Clap, Debug)]
pub struct SpecJson;

/// spec Cli command
impl SpecJson {
    pub fn exec(&self) -> Result<()> {
        // get default values for Spec
        let default_json: Spec = Default::default();
        // write data to config.json
        to_writer_pretty(&File::create("config.json")?, &default_json)?;
        Ok(())
    }
}
