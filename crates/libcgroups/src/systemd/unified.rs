use anyhow::{bail, Context, Result};
use dbus::arg::RefArg;
use std::collections::HashMap;

use super::{
    controller::Controller,
    cpu::{self, convert_shares_to_cgroup2},
    cpuset::{self, to_bitmask},
    memory, pids,
};
use crate::common::ControllerOpt;

pub struct Unified {}

impl Controller for Unified {
    fn apply(
        options: &ControllerOpt,
        systemd_version: u32,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()> {
        if let Some(unified) = options.resources.unified() {
            log::debug!("Applying unified resource restrictions");
            Self::apply(unified, systemd_version, properties)
                .context("failed to apply unified resource restrictions")?;
        }

        Ok(())
    }
}

impl Unified {
    fn apply(
        unified: &HashMap<String, String>,
        systemd_version: u32,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()> {
        for (key, value) in unified {
            match key.as_str() {
                "cpu.weight" => {
                    let shares = value
                        .parse::<u64>()
                        .with_context(|| format!("failed to parse cpu weight: {}", value))?;
                    properties.insert(cpu::CPU_WEIGHT, Box::new(convert_shares_to_cgroup2(shares)));
                }
                "cpu.max" => {
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    if parts.is_empty() || parts.len() > 2 {
                        bail!("invalid format for cpu.max: {}", value);
                    }

                    let quota = parts[0]
                        .parse::<u64>()
                        .with_context(|| format!("failed to parse cpu quota: {}", parts[0]))?;
                    properties.insert(cpu::CPU_QUOTA, Box::new(quota));

                    if parts.len() == 2 {
                        let period = parts[1].parse::<u64>().with_context(|| {
                            format!("failed to to parse cpu period: {}", parts[1])
                        })?;
                        properties.insert(cpu::CPU_PERIOD, Box::new(period));
                    }
                }
                cpuset @ ("cpuset.cpus" | "cpuset.mems") => {
                    if systemd_version <= 243 {
                        bail!(
                            "setting {} requires systemd version greater than 243",
                            cpuset
                        );
                    }

                    let bitmask = to_bitmask(value)
                        .with_context(|| format!("invalid value for cpuset.cpus: {}", value))?;

                    let systemd_cpuset = match cpuset {
                        "cpuset.cpus" => cpuset::ALLOWED_CPUS,
                        "cpuset.mems" => cpuset::ALLOWED_NODES,
                        file_name => unreachable!("{} was not matched", file_name),
                    };

                    properties.insert(systemd_cpuset, Box::new(bitmask));
                }
                memory @ ("memory.min" | "memory.low" | "memory.high" | "memory.max") => {
                    let value = value
                        .parse::<u64>()
                        .with_context(|| format!("failed to parse {}: {}", memory, value))?;
                    let systemd_memory = match memory {
                        "memory.min" => memory::MEMORY_MIN,
                        "memory.low" => memory::MEMORY_LOW,
                        "memory.high" => memory::MEMORY_HIGH,
                        "memory.max" => memory::MEMORY_MAX,
                        file_name => unreachable!("{} was not matched", file_name),
                    };
                    properties.insert(systemd_memory, Box::new(value));
                }
                "pids.max" => {
                    let pids = value.trim().parse::<i64>()?;
                    properties.insert(pids::TASKS_MAX, Box::new(pids));
                }

                unknown => log::warn!("could not apply {}. Unknown property.", unknown),
            }
        }

        Ok(())
    }
}
