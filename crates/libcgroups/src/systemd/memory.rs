use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use dbus::arg::RefArg;
use oci_spec::runtime::LinuxMemory;

use crate::common::ControllerOpt;

use super::controller::Controller;

pub const MEMORY_MIN: &str = "MemoryMin";
pub const MEMORY_LOW: &str = "MemoryLow";
pub const MEMORY_HIGH: &str = "MemoryHigh";
pub const MEMORY_MAX: &str = "MemoryMax";
pub const MEMORY_SWAP: &str = "MemorySwapMax";

pub struct Memory {}

impl Controller for Memory {
    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()> {
        if let Some(memory) = options.resources.memory() {
            log::debug!("applying memory resource restrictions");
            return Self::apply(memory, properties)
                .context("could not apply memory resource restrictions");
        }

        Ok(())
    }
}

impl Memory {
    fn apply(memory: &LinuxMemory, properties: &mut HashMap<&str, Box<dyn RefArg>>) -> Result<()> {
        if let Some(reservation) = memory.reservation() {
            match reservation {
                1..=i64::MAX => {
                    properties.insert(MEMORY_LOW, Box::new(reservation as u64));
                }
                -1 => {
                    properties.insert(MEMORY_LOW, Box::new(u64::MAX));
                }
                _ => bail!("invalid memory reservation value: {}", reservation),
            }
        }

        if let Some(limit) = memory.limit() {
            match limit {
                1..=i64::MAX => {
                    properties.insert(MEMORY_MAX, Box::new(limit as u64));
                }
                -1 => {
                    properties.insert(MEMORY_MAX, Box::new(u64::MAX));
                }
                _ => bail!("invalid memory limit value: {}", limit),
            }
        }

        Self::apply_swap(memory.swap(), memory.limit(), properties)
            .context("could not apply swap")?;
        Ok(())
    }

    // Swap needs to be converted as the runtime spec defines swap as the total of memory + swap,
    // which corresponds to memory.memsw.limit_in_bytes in cgroup v1. In v2 however swap is a
    // separate value (memory.swap.max). Therefore swap needs to be calculated from memory limit
    // and swap. Specified values could be None (no value specified), -1 (unlimited), zero or a
    // positive value. Swap needs to be bigger than the memory limit (due to swap being memory + swap)
    fn apply_swap(
        swap: Option<i64>,
        limit: Option<i64>,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()> {
        let value: Box<dyn RefArg> = match (limit, swap) {
            // memory is unlimited and swap not specified -> assume swap unlimited
            (Some(-1), None) => Box::new(u64::MAX),
            // if swap is unlimited it can be set to unlimited regardless of memory limit value
            (_, Some(-1)) => Box::new(u64::MAX),
            // if swap is zero, then it needs to be rejected regardless of memory limit value
            // as memory limit would be either bigger (invariant violation) or zero which would
            // leave the container with no memory and no swap.
            // if swap is greater than zero and memory limit is unspecified swap cannot be
            // calculated. If memory limit is zero the container would have only swap. If
            // memory is unlimited it would be bigger than swap.
            (_, Some(0)) | (None | Some(0) | Some(-1), Some(1..=i64::MAX)) => bail!(
                "cgroup v2 swap value cannot be calculated from swap of {} and limit of {}",
                swap.unwrap(),
                limit.map_or("none".to_owned(), |v| v.to_string())
            ),
            (Some(l), Some(s)) if l < s => Box::new((s - l) as u64),
            _ => return Ok(()),
        };

        properties.insert(MEMORY_SWAP, value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use dbus::arg::ArgType;
    use oci_spec::runtime::LinuxMemoryBuilder;

    use super::*;

    #[test]
    fn test_set_valid_memory_low() -> Result<()> {
        let values = vec![(536870912, 536870912u64), (-1, u64::MAX)];

        for (reservation, expected) in values {
            // arrange
            let memory = LinuxMemoryBuilder::default()
                .reservation(reservation)
                .build()
                .context("build memory spec")?;
            let mut properties: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

            // act
            Memory::apply(&memory, &mut properties).context("apply memory")?;

            // assert
            assert_eq!(properties.len(), 1);
            assert!(properties.contains_key(MEMORY_LOW));
            let memory_low = &properties[MEMORY_LOW];
            assert_eq!(memory_low.arg_type(), ArgType::UInt64);
            assert_eq!(memory_low.as_u64().unwrap(), expected);
        }

        Ok(())
    }

    #[test]
    fn test_set_valid_memory_max() -> Result<()> {
        let values = vec![(536870912, 536870912u64, 1), (-1, u64::MAX, 2)];

        for (reservation, mem_low, prop_count) in values {
            // arrange
            let memory = LinuxMemoryBuilder::default()
                .limit(reservation)
                .build()
                .context("build memory spec")?;
            let mut properties: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

            // act
            Memory::apply(&memory, &mut properties).context("apply memory")?;

            // assert
            assert_eq!(properties.len(), prop_count);
            assert!(properties.contains_key(MEMORY_MAX));
            let memory_low = &properties[MEMORY_MAX];
            assert_eq!(memory_low.arg_type(), ArgType::UInt64);
            assert_eq!(memory_low.as_u64().unwrap(), mem_low);
        }

        Ok(())
    }
}
