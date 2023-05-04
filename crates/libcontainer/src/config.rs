use crate::utils;
use oci_spec::runtime::{Hooks, Spec};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load config: {msg}")]
    SaveFailed { msg: String, path: PathBuf },
    #[error("failed to parse config: {msg}")]
    LoadFailed { msg: String, path: PathBuf },
    #[error("missing linux in spec")]
    MissingLinux,
}

type Result<T> = std::result::Result<T, ConfigError>;

const YOUKI_CONFIG_NAME: &str = "youki_config.json";

/// A configuration for passing information obtained during container creation to other commands.
/// Keeping the information to a minimum improves performance.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct YoukiConfig {
    pub hooks: Option<Hooks>,
    pub cgroup_path: PathBuf,
}

impl<'a> YoukiConfig {
    pub fn from_spec(spec: &'a Spec, container_id: &str, rootless: bool) -> Result<Self> {
        Ok(YoukiConfig {
            hooks: spec.hooks().clone(),
            cgroup_path: utils::get_cgroup_path(
                spec.linux()
                    .as_ref()
                    .ok_or(ConfigError::MissingLinux)?
                    .cgroups_path(),
                container_id,
                rootless,
            ),
        })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = fs::File::create(path.as_ref().join(YOUKI_CONFIG_NAME)).map_err(|err| {
            ConfigError::SaveFailed {
                msg: err.to_string(),
                path: path.as_ref().to_owned(),
            }
        })?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, self).map_err(|err| ConfigError::SaveFailed {
            msg: err.to_string(),
            path: path.as_ref().to_owned(),
        })?;
        writer.flush().map_err(|err| ConfigError::SaveFailed {
            msg: err.to_string(),
            path: path.as_ref().to_owned(),
        })?;

        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = fs::File::open(path.join(YOUKI_CONFIG_NAME)).map_err(|err| {
            ConfigError::LoadFailed {
                msg: err.to_string(),
                path: path.to_owned(),
            }
        })?;
        let reader = BufReader::new(file);
        let config = serde_json::from_reader(reader).map_err(|err| ConfigError::LoadFailed {
            msg: err.to_string(),
            path: path.to_owned(),
        })?;
        Ok(config)
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
        let config = YoukiConfig::from_spec(&spec, container_id, false)?;
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
        let config = YoukiConfig::from_spec(&spec, container_id, false)?;
        config.save(&tmp)?;
        let act = YoukiConfig::load(&tmp)?;
        assert_eq!(act, config);
        Ok(())
    }
}
