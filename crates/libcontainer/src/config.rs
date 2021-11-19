use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use oci_spec::runtime::{Hooks, Spec};

use crate::utils;

const YOUKI_CONFIG_NAME: &str = "youki_config.json";

/// A configuration for passing information obtained during container creation to other commands.
/// Keeping the information to a minimum improves performance.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[non_exhaustive]
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
        let file = fs::File::create(path.as_ref().join(YOUKI_CONFIG_NAME))?;
        serde_json::to_writer(&file, self)?;
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = fs::File::open(path.as_ref().join(YOUKI_CONFIG_NAME))?;
        Ok(serde_json::from_reader(&file)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::create_temp_dir;

    use super::*;
    use anyhow::Result;

    #[test]
    fn test_config_from_spec() -> Result<()> {
        let container_id = "sample";
        let spec = Spec::default();
        let config = YoukiConfig::from_spec(&spec, container_id)?;
        assert_eq!(&config.hooks, spec.hooks());
        dbg!(&config.cgroup_path);
        assert_eq!(config.cgroup_path, PathBuf::from(container_id));
        Ok(())
    }

    #[test]
    fn test_config_save_and_load() -> Result<()> {
        let container_id = "sample";
        let tmp = create_temp_dir("test_config_save_and_load").expect("create test directory");
        let spec = Spec::default();
        let config = YoukiConfig::from_spec(&spec, container_id)?;
        config.save(&tmp)?;
        let act = YoukiConfig::load(&tmp)?;
        assert_eq!(act, config);
        Ok(())
    }
}
