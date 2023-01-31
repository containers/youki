use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use dbus::arg::RefArg;
use oci_spec::runtime::LinuxCpu;

use super::controller::Controller;
use crate::common::ControllerOpt;

pub const CPU_WEIGHT: &str = "CPUWeight";
pub const CPU_QUOTA: &str = "CPUQuotaPerSecUSec";
pub const CPU_PERIOD: &str = "CPUQuotaPeriodUSec";
const MICROSECS_PER_SEC: u64 = 1_000_000;

pub(crate) struct Cpu {}

impl Controller for Cpu {
    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()> {
        if let Some(cpu) = options.resources.cpu() {
            log::debug!("Applying cpu resource restrictions");
            return Self::apply(cpu, properties)
                .context("could not apply cpu resource restrictions");
        }

        Ok(())
    }
}

impl Cpu {
    fn apply(cpu: &LinuxCpu, properties: &mut HashMap<&str, Box<dyn RefArg>>) -> Result<()> {
        if Self::is_realtime_requested(cpu) {
            bail!("realtime is not supported on systemd v2 yet");
        }

        if let Some(mut shares) = cpu.shares() {
            shares = convert_shares_to_cgroup2(shares);
            if shares != 0 {
                properties.insert(CPU_WEIGHT, Box::new(shares));
            }
        }

        // if quota is unrestricted set to 'max'
        let mut quota = u64::MAX;
        if let Some(specified_quota) = cpu.quota() {
            if specified_quota > 0 {
                let period = cpu.period().unwrap_or(100_000);

                // cpu quota in systemd must be specified as number of
                // microseconds per second of cpu time.
                quota = specified_quota as u64 * MICROSECS_PER_SEC / period;
            }
        }
        properties.insert(CPU_QUOTA, Box::new(quota));

        let mut period: u64 = 100_000;
        if let Some(specified_period) = cpu.period() {
            if specified_period > 0 {
                period = specified_period;
            }
        }
        properties.insert(CPU_PERIOD, Box::new(period));

        Ok(())
    }

    fn is_realtime_requested(cpu: &LinuxCpu) -> bool {
        cpu.realtime_period().is_some() || cpu.realtime_runtime().is_some()
    }
}

pub fn convert_shares_to_cgroup2(shares: u64) -> u64 {
    if shares == 0 {
        return 0;
    }

    1 + ((shares.saturating_sub(2)) * 9999) / 262142
}

#[cfg(test)]
mod tests {
    use dbus::arg::ArgType;
    use oci_spec::runtime::LinuxCpuBuilder;

    use super::*;

    #[test]
    fn test_set_shares() -> Result<()> {
        // arrange
        let cpu = LinuxCpuBuilder::default()
            .shares(22000u64)
            .build()
            .context("build cpu spec")?;
        let mut properties: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

        // act
        Cpu::apply(&cpu, &mut properties).context("apply cpu")?;

        // assert
        assert!(properties.contains_key(CPU_WEIGHT));

        let cpu_weight = &properties[CPU_WEIGHT];
        assert_eq!(cpu_weight.arg_type(), ArgType::UInt64);
        assert_eq!(cpu_weight.as_u64().unwrap(), 840u64);

        Ok(())
    }

    #[test]
    fn test_set_quota() -> Result<()> {
        let quotas: Vec<(i64, u64)> = vec![(200_000, 2_000_000), (0, u64::MAX), (-50000, u64::MAX)];

        for quota in quotas {
            // arrange
            let cpu = LinuxCpuBuilder::default().quota(quota.0).build().unwrap();
            let mut properties: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

            // act
            Cpu::apply(&cpu, &mut properties).context("apply cpu")?;

            // assert
            assert!(properties.contains_key(CPU_QUOTA));
            let cpu_quota = &properties[CPU_QUOTA];
            assert_eq!(cpu_quota.arg_type(), ArgType::UInt64);
            assert_eq!(cpu_quota.as_u64().unwrap(), quota.1);
        }

        Ok(())
    }

    #[test]
    fn test_set_period() -> Result<()> {
        let periods: Vec<(u64, u64)> = vec![(200_000, 200_000), (0, 100_000)];

        for period in periods {
            let cpu = LinuxCpuBuilder::default()
                .period(period.0)
                .build()
                .context("build cpu spec")?;
            let mut properties: HashMap<&str, Box<dyn RefArg>> = HashMap::new();

            // act
            Cpu::apply(&cpu, &mut properties).context("apply cpu")?;

            // assert
            assert!(properties.contains_key(CPU_PERIOD));
            let cpu_quota = &properties[CPU_PERIOD];
            assert_eq!(cpu_quota.arg_type(), ArgType::UInt64);
            assert_eq!(cpu_quota.as_u64().unwrap(), period.1);
        }

        Ok(())
    }
}
