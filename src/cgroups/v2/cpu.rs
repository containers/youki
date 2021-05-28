use std::path::Path;
use anyhow::{Result, anyhow};

use oci_spec::{LinuxCpu, LinuxResources};
use crate::{cgroups::common};

use super::controller::Controller;

const CGROUP_CPU_WEIGHT: &str = "cpu.weight";
const CGROUP_CPU_MAX: &str = "cpu.max";
const DEFAULT_PERIOD: &str = "100000";
const UNRESTRICTED_QUOTA: &str = "max";

pub struct Cpu {}

impl Controller for Cpu {
    fn apply(linux_resources: &LinuxResources, path: &Path) -> Result<()> {
        if let Some(cpu) = &linux_resources.cpu {
            Self::apply(path, cpu)?;
        }

        Ok(())
    } 
}

impl Cpu {
    fn apply(path: &Path, cpu: &LinuxCpu) -> Result<()> {
        if Self::is_realtime_requested(cpu) {
            return Err(anyhow!("realtime is not supported on cgroup v2 yet"));
        }

        if let Some(mut shares) = cpu.shares {
            shares = Self::convert_shares_to_cgroup2(shares);
            if shares != 0 { // will result in Erno 34 (numerical result out of range) otherwise
                common::write_cgroup_file(&path.join(CGROUP_CPU_WEIGHT), &shares.to_string())?;
            }
        }
        
        // if quota is unrestricted set to 'max'
        let mut quota_string = UNRESTRICTED_QUOTA.to_owned();
        if let Some(quota) = cpu.quota {
            if quota > 0 {
                quota_string = quota.to_string();
            }
        }

        let mut period_string: String = DEFAULT_PERIOD.to_owned();
        if let Some(period) = cpu.period {
            if period > 0 {
                period_string = period.to_string();
            }
        }

        // format is 'quota period'
        // the kernel default is 'max 100000'
        // 250000 250000 -> 1 CPU worth of runtime every 250ms
        // 10000 50000 -> 20% of one CPU every 50ms
        let max = quota_string + " " + &period_string;
        common::write_cgroup_file(&path.join(CGROUP_CPU_MAX), &max)?;

        Ok(())
    }

    fn convert_shares_to_cgroup2(shares: u64) -> u64{
        if shares == 0 {
            return 0;
        }

        1 + ((shares-2) * 9999)/262142
    }

    fn is_realtime_requested(cpu: &LinuxCpu) -> bool {
        if let Some(_) = cpu.realtime_period {
            return true;
        }

        if let Some(_) = cpu.realtime_runtime {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};
    use super::*;
    use crate::cgroups::test::{create_temp_dir, set_fixture, LinuxCpuBuilder};

    fn setup(testname: &str, cgroup_file: &str) -> (PathBuf, PathBuf) {
        let tmp = create_temp_dir(testname).expect("create temp directory for test");
        let cgroup_file = set_fixture(&tmp, cgroup_file, "").expect(&format!("set test fixture for {}", cgroup_file));

        (tmp, cgroup_file)
    }

    #[test]
    fn test_set_shares() {
        // arrange
        let (tmp, weight) = setup("test_set_shares", CGROUP_CPU_WEIGHT);
        let _ = set_fixture(&tmp, CGROUP_CPU_MAX, "").expect(&format!("set test fixture for {}", CGROUP_CPU_MAX));
        let cpu = LinuxCpuBuilder::new().with_shares(22000).build();
       
        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(weight).expect(&format!("read {} file content", CGROUP_CPU_WEIGHT));   
        assert_eq!(content, 840.to_string());
    }

    #[test]
    fn test_set_positive_quota() {
        // arrange
        const QUOTA: i64 = 200000;
        let (tmp, max) = setup("test_set_positive_quota", CGROUP_CPU_MAX);
        let cpu = LinuxCpuBuilder::new().with_quota(QUOTA).build();
        
        // act 
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max).expect(&format!("read {} file content", CGROUP_CPU_MAX));   
        assert_eq!(content, format!("{} {}", QUOTA, DEFAULT_PERIOD))
    }

    #[test]
    fn test_set_zero_quota() {
        // arrange
        let (tmp, max) = setup("test_set_zero_quota", CGROUP_CPU_MAX);
        let cpu = LinuxCpuBuilder::new().with_quota(0).build();
 
        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max).expect(&format!("read {} file content", CGROUP_CPU_MAX));   
        assert_eq!(content, format!("{} {}", UNRESTRICTED_QUOTA, DEFAULT_PERIOD))
    }

    #[test]
    fn test_set_positive_period() {
        // arrange
        const PERIOD: u64 = 100000;
        let (tmp, max) = setup("test_set_positive_period", CGROUP_CPU_MAX);
        let cpu = LinuxCpuBuilder::new().with_period(PERIOD).build();

        // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

        // assert
        let content = fs::read_to_string(max).expect(&format!("read {} file content", CGROUP_CPU_MAX));   
        assert_eq!(content, format!("{} {}", UNRESTRICTED_QUOTA, PERIOD))
    }

    #[test]
    fn test_set_zero_period() {
        // arrange
        let (tmp, max) = setup("test_set_zero_period", CGROUP_CPU_MAX);
        let cpu = LinuxCpuBuilder::new().with_period(0).build();

         // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

         // assert
        let content = fs::read_to_string(max).expect(&format!("read {} file content", CGROUP_CPU_MAX));   
        assert_eq!(content, format!("{} {}", UNRESTRICTED_QUOTA, DEFAULT_PERIOD));
    } 

    #[test]
    fn test_set_quota_and_period() {
        // arrange
        const QUOTA: i64= 200000;
        const PERIOD: u64 = 100000;
        let (tmp, max) = setup("test_set_quota_and_period", CGROUP_CPU_MAX);
        let cpu = LinuxCpuBuilder::new().with_quota(QUOTA).with_period(PERIOD).build();

         // act
        Cpu::apply(&tmp, &cpu).expect("apply cpu");

         // assert
        let content = fs::read_to_string(max).expect(&format!("read {} file content", CGROUP_CPU_MAX));   
        assert_eq!(content, format!("{} {}", QUOTA, PERIOD));
    }

    #[test]
    fn test_realtime_runtime_not_supported() {
        // arrange
        let tmp = create_temp_dir("test_realtime_runtime_not_supported").expect("create temp directory for test");
        let cpu = LinuxCpuBuilder::new().with_realtime_runtime(5).build();

         // act
        let result =Cpu::apply(&tmp, &cpu);

        // assert
        assert!(result.is_err(), "realtime runtime is not supported and should return an error");
    }

    #[test]
    fn test_realtime_period_not_supported() {
        // arrange
        let tmp = create_temp_dir("test_realtime_period_not_supported").expect("create temp directory for test");
        let cpu = LinuxCpuBuilder::new().with_realtime_period(5).build();

         // act
        let result =Cpu::apply(&tmp, &cpu);

        // assert
        assert!(result.is_err(), "realtime period is not supported and should return an error");
    }
}

