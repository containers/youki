use std::path::Path;

use crate::common::ControllerOpt;

pub(super) trait Controller {
    type Error;

    fn apply(controller_opt: &ControllerOpt, cgroup_path: &Path) -> Result<(), Self::Error>;
}
