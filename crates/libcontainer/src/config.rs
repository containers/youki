use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use oci_spec::runtime::{Hooks, Spec};

// TODO: comments and examples
#[derive(Clone, Deserialize, Serialize)]
pub struct YoukiConfig {
    pub hooks: Option<Hooks>,
}

impl<'a> From<&'a Spec> for YoukiConfig {
    fn from(spec: &'a Spec) -> Self {
        YoukiConfig {
            hooks: spec.hooks().clone(),
        }
    }
}

impl YoukiConfig {
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = fs::File::create(path.as_ref())?;
        serde_json::to_writer(&file, self)?;
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = fs::File::open(path.as_ref())?;
        Ok(serde_json::from_reader(&file)?)
    }
}
