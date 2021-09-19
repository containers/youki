use std::path::Path;

use anyhow::{bail, Context, Result};
use oci_spec::runtime::LinuxCpu;

use crate::{
    common::{self, ControllerOpt},
    stats::{CpuThrottling, StatsProvider},
};

use super::Controller;

const CGROUP_CPU_SHARES: &str = "cpu.shares";
const CGROUP_CPU_QUOTA: &str = "cpu.cfs_quota_us";
const CGROUP_CPU_PERIOD: &str = "cpu.cfs_period_us";
const CGROUP_CPU_RT_RUNTIME: &str = "cpu.rt_runtime_us";
const CGROUP_CPU_RT_PERIOD: &str = "cpu.rt_period_us";
const CGROUP_CPU_STAT: &str = "cpu.stat";

pub struct Cpu {}

impl Controller for Cpu {
    type Resource = LinuxCpu;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Cpu cgroup config");

        if let Some(cpu) = Self::needs_to_handle(controller_opt) {
            Self::apply(cgroup_root, cpu).context("failed to apply cpu resource restrictions")?;
        }

        Ok(())
    }

    fn needs_to_handle(controller_opt: &ControllerOpt) -> Option<&Self::Resource> {
        if let Some(cpu) = &controller_opt.resources.cpu() {
            if cpu.shares().is_some()
                || cpu.period().is_some()
                || cpu.quota().is_some()
                || cpu.realtime_period().is_some()
                || cpu.realtime_runtime().is_some()
            {
                return Some(cpu);
            }
        }

        None
    }
}

impl StatsProvider for Cpu {
    type Stats = CpuThrottling;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let mut stats = CpuThrottling::default();
        let stat_path = cgroup_path.join(CGROUP_CPU_STAT);
        let stat_content = common::read_cgroup_file(&stat_path)?;

        let parts: Vec<&str> = stat_content.split_ascii_whitespace().collect();
        if parts.len() < 6 {
            bail!(
                "{} contains less than the expected number of entries",
                stat_path.display()
            );
        }

        if parts[0] != "nr_periods" {
            bail!(
                "{} does not contain the number of elapsed periods",
                stat_path.display()
            );
        }

        if parts[2] != "nr_throttled" {
            bail!(
                "{} does not contain the number of throttled periods",
                stat_path.display()
            );
        }

        if parts[4] != "throttled_time" {
            bail!(
                "{} does not contain the total time tasks have spent throttled",
                stat_path.display()
            );
        }

        stats.periods = parts[1].parse().context("failed to parse nr_periods")?;
        stats.throttled_periods = parts[3].parse().context("failed to parse nr_throttled")?;
        stats.throttled_time = parts[5].parse().context("failed to parse throttled time")?;

        Ok(stats)
    }
}

impl Cpu {
    fn apply(root_path: &Path, cpu: &LinuxCpu) -> Result<()> {
        if let Some(cpu_shares) = cpu.shares() {
            if cpu_shares != 0 {
                common::write_cgroup_file(root_path.join(CGROUP_CPU_SHARES), cpu_shares)?;
            }
        }

        if let Some(cpu_period) = cpu.period() {
            if cpu_period != 0 {
                common::write_cgroup_file(root_path.join(CGROUP_CPU_PERIOD), cpu_period)?;
            }
        }

        if let Some(cpu_quota) = cpu.quota() {
            if cpu_quota != 0 {
                common::write_cgroup_file(root_path.join(CGROUP_CPU_QUOTA), cpu_quota)?;
            }
        }

        if let Some(rt_runtime) = cpu.realtime_runtime() {
            if rt_runtime != 0 {
                common::write_cgroup_file(root_path.join(CGROUP_CPU_RT_RUNTIME), rt_runtime)?;
            }
        }

        if let Some(rt_period) = cpu.realtime_period() {
            if rt_period != 0 {
                common::write_cgroup_file(root_path.join(CGROUP_CPU_RT_PERIOD), rt_period)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture, setup};
    use oci_spec::runtime::LinuxCpuBuilder;
    use std::fs;

    #[test]
    fn test_set_shares() {
        // arrange
        let (tmp, shares) = setup("test_set_shares", CGROUP_CPU_SHARES);
        let _ = set_fixture(&tmp, CGROUP_CPU_SHARES, "")
            .unwrap_or_else(|_| panic!("set test fixture for {}", CGROUP_CPU_SHARES));
        let cpu = LinuxCpuBuilder::default().shares(2048u64).build().unwrap();

        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(shares)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPU_SHARES));
        assert_eq!(content, 2048.to_string());
    }

    #[test]
    fn test_set_quota() {
        // arrange
        const QUOTA: i64 = 200000;
        let (tmp, max) = setup("test_set_quota", CGROUP_CPU_QUOTA);
        let cpu = LinuxCpuBuilder::default().quota(QUOTA).build().unwrap();

        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPU_QUOTA));
        assert_eq!(content, QUOTA.to_string());
    }

    #[test]
    fn test_set_period() {
        // arrange
        const PERIOD: u64 = 100000;
        let (tmp, max) = setup("test_set_period", CGROUP_CPU_PERIOD);
        let cpu = LinuxCpuBuilder::default().period(PERIOD).build().unwrap();

        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPU_PERIOD));
        assert_eq!(content, PERIOD.to_string());
    }

    #[test]
    fn test_set_rt_runtime() {
        // arrange
        const RUNTIME: i64 = 100000;
        let (tmp, max) = setup("test_set_rt_runtime", CGROUP_CPU_RT_RUNTIME);
        let cpu = LinuxCpuBuilder::default()
            .realtime_runtime(RUNTIME)
            .build()
            .unwrap();

        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPU_RT_RUNTIME));
        assert_eq!(content, RUNTIME.to_string());
    }

    #[test]
    fn test_set_rt_period() {
        // arrange
        const PERIOD: u64 = 100000;
        let (tmp, max) = setup("test_set_rt_period", CGROUP_CPU_RT_PERIOD);
        let cpu = LinuxCpuBuilder::default()
            .realtime_period(PERIOD)
            .build()
            .unwrap();

        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_CPU_RT_PERIOD));
        assert_eq!(content, PERIOD.to_string());
    }

    #[test]
    fn test_stat_cpu_throttling() {
        let tmp = create_temp_dir("test_stat_cpu_throttling").expect("create test directory");
        let stat_content = &[
            "nr_periods 165000",
            "nr_throttled 27",
            "throttled_time 1080",
        ]
        .join("\n");
        set_fixture(&tmp, CGROUP_CPU_STAT, stat_content).expect("create stat file");

        let actual = Cpu::stats(&tmp).expect("get cgroup stats");
        let expected = CpuThrottling {
            periods: 165000,
            throttled_periods: 27,
            throttled_time: 1080,
        };
        assert_eq!(actual, expected);
    }
}
