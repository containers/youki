use std::collections::HashMap;

use anyhow::Result;
use dbus::arg::RefArg;

use crate::common::ControllerOpt;

pub(crate) trait Controller {
    fn apply(
        options: &ControllerOpt,
        systemd_version: u32,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()>;
}
