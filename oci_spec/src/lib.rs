use anyhow::{bail, Context, Result};
use caps::Capability;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};

mod linux;
mod miscellaneous;
mod process;
mod test;

// re-export for ease of use
pub use linux::*;
pub use miscellaneous::*;
pub use process::*;

// Base configuration for the container
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Spec {
    // Version of the Open Container Initiative Runtime Specification with which the bundle complies
    #[serde(default, rename = "ociVersion")]
    pub version: String,
    // Computer os and arch
    pub platform: Option<Platform>,
    // Configures container process
    pub process: Process,
    // Configures container's root filesystem
    pub root: Root,
    // Configures container's hostname
    #[serde(default)]
    pub hostname: String,
    // Configures additional mounts (on top of Root)
    #[serde(default)]
    pub mounts: Vec<Mount>,
    // Arbitrary metadata for container
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    // Platform specific config for Linux based containers
    pub linux: Option<Linux>,
}

// This gives a basic boilerplate for Spec that can be used calling Default::default().
// The values given are similar to the defaults seen in docker and runc, it creates a containerized shell!
// (see respective types default impl for more info)
impl Default for Spec {
    fn default() -> Self {
        Spec {
            // Defaults to most current oci version
            version: String::from("1.0.2-dev"),
            platform: Some(Default::default()),
            process: Default::default(),
            root: Default::default(),
            // Defaults hostname as youki
            hostname: String::from("youki"),
            mounts: get_default_mounts(),
            // Defaults to empty metadata
            annotations: Default::default(),
            linux: Some(Default::default()),
        }
    }
}

impl Spec {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = fs::File::open(path)
            .with_context(|| format!("load spec: failed to open {:?}", path))?;
        let spec: Spec = serde_json::from_reader(&file)?;
        Ok(spec)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let file = fs::File::create(path)
            .with_context(|| format!("save spec: failed to create/open {:?}", path))?;
        serde_json::to_writer(&file, self)
            .with_context(|| format!("failed to save spec to {:?}", path))?;

        Ok(())
    }

    pub fn canonicalize_rootfs<P: AsRef<Path>>(&mut self, bundle: P) -> Result<()> {
        let canonical_root_path = if self.root.path.is_absolute() {
            fs::canonicalize(&self.root.path)
                .with_context(|| format!("failed to canonicalize {:?}", self.root.path))?
        } else {
            let canonical_bundle_path = fs::canonicalize(&bundle).context(format!(
                "failed to canonicalize bundle: {:?}",
                bundle.as_ref()
            ))?;

            fs::canonicalize(&canonical_bundle_path.join(&self.root.path)).context(format!(
                "failed to canonicalize rootfs: {:?}",
                &self.root.path
            ))?
        };
        self.root.path = canonical_root_path;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

    #[test]
    fn test_canonicalize_rootfs() -> Result<()> {
        Ok(())
    }

    #[test]
    fn test_load_save() -> Result<()> {
        let spec = Spec{..Default::default()};
        let test_dir = tempfile::tempdir().with_context(|| "Failed to create tmp test dir")?;
        let spec_path = test_dir.into_path().join("config.json");
        
        // Test first save the default config, and then load the saved config.
        // The before and after should be the same.
        spec.save(&spec_path).with_context(|| "Failed to save spec")?;
        let loaded_spec = Spec::load(&spec_path).with_context(|| "Failed to load the saved spec.")?;
        assert_eq!(spec, loaded_spec, "The saved spec is not the same as the loaded spec");

        Ok(())
    }
}