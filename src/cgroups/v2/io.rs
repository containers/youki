use anyhow::Result;

use super::controller::Controller;
use oci_spec::LinuxResources;

pub struct Io {}

impl Controller for Io {
    fn apply(_: &LinuxResources, _: &std::path::Path) -> Result<()> {
        Ok(())
    }
}
