use std::path::Path;
use anyhow::Result;

use oci_spec::{LinuxCpu, LinuxResources};
use crate::{cgroups::common};

use super::controller::Controller;

const CGROUP_CPU_WEIGHT: &str = "cpu.weight";
const CGROUP_CPU_MAX: &str = "cpu.max";

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
        if let Some(mut shares) = cpu.shares {
            shares = Self::convert_share_to_cgroup2(shares);
            if shares != 0 { // will result in Erno 34 (numerical result out of range) otherwise
                common::write_cgroup_file(&path.join(CGROUP_CPU_WEIGHT), &shares.to_string())?;
            }
        }

        // if quota is unrestricted set to 'max'
        let mut quota_string = "max".to_owned();
        if let Some(quota) = cpu.quota {
            if quota > 0 {
                quota_string = quota.to_string();
            }
        }

        let mut period_string: String = "".to_owned();
        if let Some(period) = cpu.period {
            if period == 0 {
                period_string = 100000.to_string();
            } else {
                period_string = period.to_string()
            }
        }

        // format is 'quota period'
        // the kernel default is 'max 100000'
        let max = quota_string + " " + &period_string;
        common::write_cgroup_file(&path.join(CGROUP_CPU_MAX), &max)?;

        Ok(())
    }

    fn convert_share_to_cgroup2(shares: u64) -> u64{
        if shares == 0 {
            return 0;
        }

        1 + ((shares-2) * 9999)/262142
    }
}

#[cfg(test)]
mod tests {

    
}

