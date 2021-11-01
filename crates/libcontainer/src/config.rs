use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use oci_spec::runtime::{Hooks, Spec};

use crate::utils;

// TODO: comments and examples
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct YoukiConfig {
    pub hooks: Option<Hooks>,
    pub cgroup_path: PathBuf,
}

impl<'a> YoukiConfig {
    pub fn from_spec(spec: &'a Spec, container_id: &str) -> Result<Self> {
        Ok(YoukiConfig {
            hooks: spec.hooks().clone(),
            cgroup_path: utils::get_cgroup_path(
                spec.linux()
                    .as_ref()
                    .context("no linux in spec")?
                    .cgroups_path(),
                container_id,
            ),
        })
    }

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
