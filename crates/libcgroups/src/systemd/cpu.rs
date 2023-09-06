use std::collections::HashMap;

use super::dbus_native::serialize::DbusSerialize;
use oci_spec::runtime::LinuxCpu;

use super::controller::Controller;
use crate::common::ControllerOpt;

pub const CPU_WEIGHT: &str = "CPUWeight";
pub const CPU_QUOTA: &str = "CPUQuotaPerSecUSec";
pub const CPU_PERIOD: &str = "CPUQuotaPeriodUSec";
const MICROSECS_PER_SEC: u64 = 1_000_000;

#[derive(thiserror::Error, Debug)]
pub enum SystemdCpuError {
    #[error("realtime is not supported on systemd v2 yet")]
    RealtimeSystemd,
}

pub(crate) struct Cpu {}

impl Controller for Cpu {
    type Error = SystemdCpuError;

    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<&str, Box<dyn DbusSerialize>>,
    ) -> Result<(), Self::Error> {
        if let Some(cpu) = options.resources.cpu() {
            tracing::debug!("Applying cpu resource restrictions");
            Self::apply(cpu, properties)?;
        }

        Ok(())
    }
}

impl Cpu {
    fn apply(
        cpu: &LinuxCpu,
        properties: &mut HashMap<&str, Box<dyn DbusSerialize>>,
    ) -> Result<(), SystemdCpuError> {
        if Self::is_realtime_requested(cpu) {
            return Err(SystemdCpuError::RealtimeSystemd);
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
    use anyhow::{Context, Result};
    use dbus::arg::ArgType;
    use oci_spec::runtime::LinuxCpuBuilder;

    use crate::recast;

    use super::*;

    #[test]
    fn test_set_shares() -> Result<()> {
        // arrange
        let cpu = LinuxCpuBuilder::default()
            .shares(22000u64)
            .build()
            .context("build cpu spec")?;
        let mut properties: HashMap<&str, Box<dyn DbusSerialize>> = HashMap::new();

        // act
        Cpu::apply(&cpu, &mut properties)?;

        // assert
        assert!(properties.contains_key(CPU_WEIGHT));

        let cpu_weight = &properties[CPU_WEIGHT];
        let val = recast!(cpu_weight, u64)?;
        assert_eq!(val, 840u64);

        Ok(())
    }

    #[test]
    fn test_set_quota() -> Result<()> {
        let quotas: Vec<(i64, u64)> = vec![(200_000, 2_000_000), (0, u64::MAX), (-50000, u64::MAX)];

        for quota in quotas {
            // arrange
            let cpu = LinuxCpuBuilder::default().quota(quota.0).build().unwrap();
            let mut properties: HashMap<&str, Box<dyn DbusSerialize>> = HashMap::new();

            // act
            Cpu::apply(&cpu, &mut properties)?;

            // assert
            assert!(properties.contains_key(CPU_QUOTA));
            let cpu_quota = &properties[CPU_QUOTA];
            let val = recast!(cpu_quota, u64)?;
            assert_eq!(val, quota.1);
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
            let mut properties: HashMap<&str, Box<dyn DbusSerialize>> = HashMap::new();

            // act
            Cpu::apply(&cpu, &mut properties)?;

            // assert
            assert!(properties.contains_key(CPU_PERIOD));
            let cpu_quota = &properties[CPU_PERIOD];
            let val = recast!(cpu_quota, u64)?;
            assert_eq!(val, period.1);
        }

        Ok(())
    }
}
