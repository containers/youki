use std::{collections::HashMap, convert::Infallible};

use dbus::arg::RefArg;
use oci_spec::runtime::LinuxPids;

use crate::common::ControllerOpt;

use super::controller::Controller;

pub const TASKS_MAX: &str = "TasksMax";

pub struct Pids {}

impl Controller for Pids {
    type Error = Infallible;

    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<(), Self::Error> {
        if let Some(pids) = options.resources.pids() {
            log::debug!("Applying pids resource restrictions");
            Self::apply(pids, properties);
        }

        Ok(())
    }
}

impl Pids {
    fn apply(pids: &LinuxPids, properties: &mut HashMap<&str, Box<dyn RefArg>>) {
        let limit = if pids.limit() > 0 {
            pids.limit() as u64
        } else {
            u64::MAX
        };

        properties.insert(TASKS_MAX, Box::new(limit));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, Context, Result};
    use dbus::arg::ArgType;
    use oci_spec::runtime::{LinuxPidsBuilder, LinuxResources, LinuxResourcesBuilder};

    fn setup(resources: &LinuxResources) -> (ControllerOpt, HashMap<&str, Box<dyn RefArg>>) {
        let properties = HashMap::new();
        let options = ControllerOpt {
            resources,
            disable_oom_killer: false,
            oom_score_adj: None,
            freezer_state: None,
        };

        (options, properties)
    }

    #[test]
    fn test_pids_positive_limit() -> Result<()> {
        let resources = LinuxResourcesBuilder::default()
            .pids(LinuxPidsBuilder::default().limit(10).build()?)
            .build()?;
        let (options, mut properties) = setup(&resources);

        <Pids as Controller>::apply(&options, 245, &mut properties)
            .map_err(|err| anyhow!(err))
            .context("apply pids")?;

        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key(TASKS_MAX));

        let task_max = properties.get(TASKS_MAX).unwrap();
        assert_eq!(task_max.arg_type(), ArgType::UInt64);
        assert_eq!(task_max.as_u64().unwrap(), 10);

        Ok(())
    }

    #[test]
    fn test_pids_zero_limit() -> Result<()> {
        let resources = LinuxResourcesBuilder::default()
            .pids(LinuxPidsBuilder::default().limit(0).build()?)
            .build()?;
        let (options, mut properties) = setup(&resources);

        <Pids as Controller>::apply(&options, 245, &mut properties)
            .map_err(|err| anyhow!(err))
            .context("apply pids")?;

        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key(TASKS_MAX));

        let task_max = properties.get(TASKS_MAX).unwrap();
        assert_eq!(task_max.arg_type(), ArgType::UInt64);
        assert_eq!(task_max.as_u64().unwrap(), u64::MAX);

        Ok(())
    }

    #[test]
    fn test_pids_negative_limit() -> Result<()> {
        let resources = LinuxResourcesBuilder::default()
            .pids(LinuxPidsBuilder::default().limit(-500).build()?)
            .build()?;
        let (options, mut properties) = setup(&resources);

        <Pids as Controller>::apply(&options, 245, &mut properties)
            .map_err(|err| anyhow!(err))
            .context("apply pids")?;

        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key(TASKS_MAX));

        let task_max = properties.get(TASKS_MAX).unwrap();
        assert_eq!(task_max.arg_type(), ArgType::UInt64);
        assert_eq!(task_max.as_u64().unwrap(), u64::MAX);

        Ok(())
    }
}
