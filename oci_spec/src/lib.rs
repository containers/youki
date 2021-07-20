use anyhow::{bail, Context, Result};
use caps::Capability;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
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
        let file =
            File::open(path).with_context(|| format!("load spec: failed to open {:?}", path))?;
        let spec: Spec = serde_json::from_reader(&file)?;
        Ok(spec)
    }

    pub fn canonicalize_rootfs(&mut self) -> Result<()> {
        self.root.path = std::fs::canonicalize(&self.root.path)
            .with_context(|| format!("failed to canonicalize {:?}", self.root.path))?;
        Ok(())
    }
}
