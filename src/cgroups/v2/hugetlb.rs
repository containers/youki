use anyhow::Result;

use super::controller::Controller;
use oci_spec::LinuxResources;

pub struct HugeTlb {}

impl Controller for HugeTlb {
    fn apply(_: &LinuxResources, _: &std::path::Path) -> Result<()> {
        Ok(())
    }
}
