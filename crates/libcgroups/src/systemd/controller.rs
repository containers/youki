use std::collections::HashMap;

use super::dbus_native::serialize::DbusSerialize;

use crate::common::ControllerOpt;

pub(super) trait Controller {
    type Error;

    fn apply(
        options: &ControllerOpt,
        systemd_version: u32,
        properties: &mut HashMap<&str, Box<dyn DbusSerialize>>,
    ) -> Result<(), Self::Error>;
}
