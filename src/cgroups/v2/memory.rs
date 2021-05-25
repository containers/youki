use std::path::Path;
use anyhow::Result;

use oci_spec::{LinuxMemory, LinuxResources};

use super::controller::Controller;

pub struct Memory {}

impl Controller for Memory {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        match &linux_resources.memory {
            None => return Ok(()),
            Some(memory) => Self::apply(cgroup_path, memory)?,
        }

        Ok(())
    }
}

impl Memory {
    fn apply(path: &Path, memory: &LinuxMemory) -> Result<()> {
        todo!();
    }
}