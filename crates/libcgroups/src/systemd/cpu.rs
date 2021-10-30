use std::collections::HashMap;

use anyhow::{bail, Result};
use dbus::arg::RefArg;
use oci_spec::runtime::LinuxCpu;

use super::controller::Controller;
use crate::common::ControllerOpt;

pub(crate) struct Cpu {}

impl Controller for Cpu {
    fn apply(
        options: &ControllerOpt,
        properties: &mut HashMap<String, Box<dyn RefArg>>,
    ) -> Result<()> {
        if let Some(cpu) = options.resources.cpu() {
            return Self::apply(cpu, properties);
        }

        Ok(())
    }
}

impl Cpu {
    fn apply(cpu: &LinuxCpu, properties: &mut HashMap<String, Box<dyn RefArg>>) -> Result<()> {
        if Self::is_realtime_requested(cpu) {
            bail!("realtime is not supported on cgroup v2 yet");
        }

        if let Some(mut shares) = cpu.shares() {
            shares = Self::convert_shares_to_cgroup2(shares);
            if shares != 0 {
                properties.insert("CPUWeight".to_owned(), Box::new(shares));
            }
        }

        // if quota is unrestricted set to 'max'
        let mut quota = u64::MAX;
        if let Some(specified_quota) = cpu.quota() {
            if specified_quota > 0 {
                quota = specified_quota as u64
            }
        }
        properties.insert("CPUQuotaPerSecUSec".to_owned(), Box::new(quota));

        let mut period: u64 = 100_000;
        if let Some(specified_period) = cpu.period() {
            if specified_period > 0 {
                period = specified_period;
            }
        }
        properties.insert("CPUQuotaPeriodUSec".to_owned(), Box::new(period));

        Ok(())
    }

    fn is_realtime_requested(cpu: &LinuxCpu) -> bool {
        if cpu.realtime_period().is_some() {
            return true;
        }

        if cpu.realtime_runtime().is_some() {
            return true;
        }

        false
    }

    fn convert_shares_to_cgroup2(shares: u64) -> u64 {
        if shares == 0 {
            return 0;
        }

        1 + ((shares - 2) * 9999) / 262142
    }
}
