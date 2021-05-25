use std::path::Path;
use anyhow::Result;

use oci_spec::{LinuxCpu, LinuxResources};

use super::controller::Controller;

const CGROUP_CPU_WEIGHT: &str = "cpu.weight";
const CGROUP_CPU_MAX: &str = "cpu.max";

pub struct Cpu {}

impl Controller for Cpu {
    fn apply(linux_resources: &LinuxResources, path: &Path) -> Result<()> {
        match &linux_resources.cpu {
            None => return Ok(()),
            Some(cpu) => Self::apply(path, cpu)?, 
        }

        Ok(())
    } 
}

impl Cpu {
    fn apply(path: &Path, cpu: &LinuxCpu) -> Result<()> {
        Ok(())
    }
}

