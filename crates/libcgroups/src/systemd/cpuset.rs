use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use fixedbitset::FixedBitSet;
use oci_spec::runtime::LinuxCpu;

use crate::common::ControllerOpt;

use super::controller::Controller;

pub struct CpuSet {}

impl Controller for CpuSet {
    fn apply(
        options: &ControllerOpt,
        systemd_version: u32,
        properties: &mut HashMap<String, Box<dyn dbus::arg::RefArg>>,
    ) -> Result<()> {
        if let Some(cpu) = options.resources.cpu() {
            log::debug!("Applying cpuset resource restrictions");
            return Self::apply(cpu, systemd_version, properties)
                .context("could not apply cpuset resource restrictions");
        }

        Ok(())
    }
}

impl CpuSet {
    fn apply(
        cpu: &LinuxCpu,
        systemd_version: u32,
        properties: &mut HashMap<String, Box<dyn dbus::arg::RefArg>>,
    ) -> Result<()> {
        if systemd_version < 244 {
            bail!(
                "Systemd version ({}) is too old to support cpuset restrictions",
                systemd_version
            );
        }

        if let Some(cpus) = cpu.cpus() {
            let cpu_mask = Self::to_bitmask(cpus).context("could not create bitmask for cpus")?;
            properties.insert("AllowedCPUs".to_owned(), Box::new(cpu_mask));
        }

        if let Some(mems) = cpu.mems() {
            let mems_mask =
                Self::to_bitmask(mems).context("could not create bitmask for memory nodes")?;
            properties.insert("AllowedMemoryNodes".to_owned(), Box::new(mems_mask));
        }

        Ok(())
    }

    fn to_bitmask(range: &str) -> Result<Vec<u8>> {
        let mut bitset = FixedBitSet::with_capacity(8);

        for cpu_set in range.split_terminator(',') {
            let cpu_set = cpu_set.trim();
            if cpu_set.is_empty() {
                continue;
            }

            let cpus: Vec<&str> = cpu_set.split('-').collect();
            if cpus.len() == 1 {
                let cpu_index: usize = cpus[0].parse()?;
                if cpu_index >= bitset.len() {
                    bitset.grow(bitset.len() + 8);
                }
                bitset.set(cpu_index, true);
            } else {
                let start_index = cpus[0].parse()?;
                let end_index = cpus[1].parse()?;
                if start_index > end_index {
                    bail!("invalid cpu range {}", cpu_set);
                }

                if end_index >= bitset.len() {
                    bitset.grow(end_index + 1);
                }

                bitset.set_range(start_index..end_index + 1, true);
            }
        }

        // systemd expects a sequence of bytes with no leading zeros, otherwise the values will not be set
        // with no error message
        Ok(bitset
            .as_slice()
            .iter()
            .flat_map(|b| b.to_be_bytes())
            .skip_while(|b| *b == 0u8)
            .collect())
    }
}
