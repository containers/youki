use anyhow::Result;

use super::controller::Controller;
use oci_spec::LinuxResources;

pub struct Pids {}

impl Controller for Pids {
    fn apply(_: &LinuxResources, _: &std::path::Path) -> Result<()> {
        Ok(())
    }
}
