pub mod bpf;
mod controller;
pub mod emulator;
pub mod program;

use crate::v2::controller::Controller;
use anyhow::Result;
use oci_spec::LinuxResources;
use std::path::Path;

pub struct Devices {}

impl Controller for Devices {
    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        #[cfg(not(feature = "cgroupsv2_devices"))]
        return Ok(());

        #[cfg(feature = "cgroupsv2_devices")]
        return controller::Devices::apply(linux_resources, cgroup_root);
    }
}
