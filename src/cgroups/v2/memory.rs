use anyhow::Result;
use std::path::Path;

use oci_spec::{LinuxMemory, LinuxResources};

use super::controller::Controller;

pub struct Memory {}

impl Controller for Memory {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        if let Some(memory) = &linux_resources.memory {
            Self::apply(cgroup_path, memory)?;
        }

        Ok(())
    }
}

impl Memory {
    fn apply(_: &Path, _: &LinuxMemory) -> Result<()> {
        Ok(())
    }
}
