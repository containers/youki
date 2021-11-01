use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use dbus::arg::RefArg;
use oci_spec::runtime::LinuxCpu;

use super::controller::Controller;
use crate::common::ControllerOpt;

pub(crate) struct Cpu {}

impl Controller for Cpu {
    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<String, Box<dyn RefArg>>,
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
    fn apply(cpu: &LinuxCpu, properties: &mut HashMap<String, Box<dyn RefArg>>) -> Result<()> {
        if Self::is_realtime_requested(cpu) {
            bail!("realtime is not supported on systemd v2 yet");
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
        let mut properties: HashMap<String, Box<dyn RefArg>> = HashMap::new();

        // act
        Cpu::apply(&cpu, &mut properties).context("apply cpu")?;

        // assert
        assert!(properties.contains_key("CPUWeight"));

        let cpu_weight = &properties["CPUWeight"];
        assert_eq!(cpu_weight.arg_type(), ArgType::UInt64);
        assert_eq!(cpu_weight.as_u64().unwrap(), 840u64);

        Ok(())
    }

    #[test]
    fn test_set_quota() -> Result<()> {
        let quotas: Vec<(i64, u64)> = vec![(200_000, 200_000), (0, u64::MAX), (-50000, u64::MAX)];

        for quota in quotas {
            // arrange
            let cpu = LinuxCpuBuilder::default().quota(quota.0).build().unwrap();
            let mut properties: HashMap<String, Box<dyn RefArg>> = HashMap::new();

            // act
            Cpu::apply(&cpu, &mut properties).context("apply cpu")?;

            // assert
            assert!(properties.contains_key("CPUQuotaPerSecUSec"));
            let cpu_quota = &properties["CPUQuotaPerSecUSec"];
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
            let mut properties: HashMap<String, Box<dyn RefArg>> = HashMap::new();

            // act
            Cpu::apply(&cpu, &mut properties).context("apply cpu")?;

            // assert
            assert!(properties.contains_key("CPUQuotaPeriodUSec"));
            let cpu_quota = &properties["CPUQuotaPeriodUSec"];
            assert_eq!(cpu_quota.arg_type(), ArgType::UInt64);
            assert_eq!(cpu_quota.as_u64().unwrap(), period.1);
        }

        Ok(())
    }
}
