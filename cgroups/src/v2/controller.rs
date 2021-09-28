use anyhow::Result;
use std::path::Path;

use crate::common::ControllerOpt;

pub trait Controller {
    fn apply(controller_opt: &ControllerOpt, cgroup_path: &Path) -> Result<()>;
}
