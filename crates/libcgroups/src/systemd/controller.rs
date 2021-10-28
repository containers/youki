use anyhow::Result;

use crate::common::ControllerOpt;

pub(crate) trait Controller {
    fn apply(resources: &ControllerOpt) -> Result<()>;
}
