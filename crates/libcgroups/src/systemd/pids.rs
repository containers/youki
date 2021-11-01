use std::collections::HashMap;

use anyhow::{Context, Result};
use dbus::arg::RefArg;
use oci_spec::runtime::LinuxPids;

use crate::common::ControllerOpt;

use super::controller::Controller;

pub struct Pids {}

impl Controller for Pids {
    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<String, Box<dyn RefArg>>,
    ) -> Result<()> {
        if let Some(pids) = options.resources.pids() {
            log::debug!("Applying pids resource restrictions");
            return Self::apply(pids, properties).context("");
        }

        Ok(())
    }
}

impl Pids {
    fn apply(pids: &LinuxPids, properties: &mut HashMap<String, Box<dyn RefArg>>) -> Result<()> {
        let limit = if pids.limit() > 0 {
            pids.limit() as u64
        } else {
            u64::MAX
        };

        properties.insert("TasksMax".to_owned(), Box::new(limit));
        Ok(())
    }
}
