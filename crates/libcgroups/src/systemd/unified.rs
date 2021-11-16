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
                    properties.insert(pids::TASKS_MAX, Box::new(pids as u64));
                }

                unknown => log::warn!("could not apply {}. Unknown property.", unknown),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use dbus::arg::ArgType;

    use super::*;

    #[test]
    fn test_set() -> Result<()> {
        // arrange
        let unified: HashMap<String, String> = [
            ("cpu.weight", "22000"),
            ("cpuset.cpus", "0-3"),
            ("cpuset.mems", "0-3"),
            ("memory.min", "100000"),
            ("memory.low", "200000"),
            ("memory.high", "300000"),
            ("memory.max", "400000"),
            ("pids.max", "100"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();

        let mut expected: HashMap<&str, Box<dyn RefArg>> = HashMap::new();
        expected.insert(cpu::CPU_WEIGHT, Box::new(840 as u64));
        expected.insert(cpuset::ALLOWED_CPUS, Box::new(vec![15 as u64]));
        expected.insert(cpuset::ALLOWED_NODES, Box::new(vec![15 as u64]));
        expected.insert(memory::MEMORY_MIN, Box::new(100000 as u64));
        expected.insert(memory::MEMORY_LOW, Box::new(200000 as u64));
        expected.insert(memory::MEMORY_HIGH, Box::new(300000 as u64));
        expected.insert(memory::MEMORY_MAX, Box::new(400000 as u64));
        expected.insert(pids::TASKS_MAX, Box::new(100 as u64));

        // act
        let mut actual: HashMap<&str, Box<dyn RefArg>> = HashMap::new();
        Unified::apply(&unified, 245, &mut actual).context("apply unified")?;

        // assert
        for (setting, value) in expected {
            assert!(actual.contains_key(setting));
            assert_eq!(value.arg_type(), actual[setting].arg_type(), "{}", setting);
            match value.arg_type() {
                ArgType::UInt64 => {
                    assert_eq!(value.as_u64(), actual[setting].as_u64(), "{}", setting)
                }
                ArgType::Array => assert_eq!(
                    value.as_iter().unwrap().next().unwrap().as_u64(),
                    actual[setting].as_iter().unwrap().next().unwrap().as_u64()
                ),
                arg_type => bail!("unexpected arg type {:?}", arg_type),
            }
        }

        Ok(())
    }

    #[test]
    fn test_cpu_max_quota_and_period() -> Result<()> {
        // arrange
        let unified: HashMap<String, String> = [
            ("cpu.max", "500000 250000"),        
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let mut actual: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

        // act
        Unified::apply(&unified, 245, &mut actual).context("apply unified")?;

        // assert
        assert!(actual.contains_key(cpu::CPU_PERIOD));
        assert!(actual.contains_key(cpu::CPU_QUOTA));

        assert_eq!(actual[cpu::CPU_PERIOD].arg_type(), ArgType::UInt64);
        assert_eq!(actual[cpu::CPU_QUOTA].arg_type(), ArgType::UInt64);

        assert_eq!(actual[cpu::CPU_PERIOD].as_u64().unwrap(), 250000);
        assert_eq!(actual[cpu::CPU_QUOTA].as_u64().unwrap(), 500000);

        Ok(())
    }

    #[test]
    fn test_cpu_max_quota_only() -> Result<()> {
        // arrange
        let unified: HashMap<String, String> = [
            ("cpu.max", "500000"),        
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let mut actual: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

        // act
        Unified::apply(&unified, 245, &mut actual).context("apply unified")?;

        // assert
        assert!(!actual.contains_key(cpu::CPU_PERIOD));
        assert!(actual.contains_key(cpu::CPU_QUOTA));

        assert_eq!(actual[cpu::CPU_QUOTA].arg_type(), ArgType::UInt64);
        assert_eq!(actual[cpu::CPU_QUOTA].as_u64().unwrap(), 500000);

        Ok(())
    }
}
