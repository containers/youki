use anyhow::Result;
use oci_spec::runtime::LinuxResources;

use crate::common::ControllerOpt;

use super::controller::Controller;

pub(crate) struct Cpu {}

impl Controller for Cpu {
    fn apply(options: &ControllerOpt) -> Result<()> {
        Ok(())
    }
}
