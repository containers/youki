use std::collections::HashMap;

use anyhow::Result;
use dbus::arg::RefArg;

use crate::common::ControllerOpt;

pub(crate) trait Controller {
    fn apply(
        resources: &ControllerOpt,
        properties: &mut HashMap<String, Box<dyn RefArg>>,
    ) -> Result<()>;
}
