use std::path::Path;

use anyhow::{bail, Context, Result};
use oci_spec::LinuxResources;

use crate::cgroups::{
    common,
    stats::{CpuUsage, StatsProvider},
};

use super::Controller;

// Contains user mode and kernel mode cpu consumption
const CGROUP_CPUACCT_STAT: &str = "cpuacct.stat";
// Contains overall cpu consumption
const CGROUP_CPUACCT_USAGE: &str = "cpuacct.usage";
// Contains user mode and kernel mode cpu consumption differentiated by core
const CGROUP_CPUACCT_USAGE_ALL: &str = "cpuacct.usage_all";
// Contains overall cpu consumption differentiated by core
const CGROUP_CPUACCT_PERCPU: &str = "cpuacct.usage_percpu";

pub struct CpuAcct {}

impl Controller for CpuAcct {
    type Resource = ();

    fn apply(_linux_resources: &LinuxResources, _cgroup_path: &Path) -> Result<()> {
        Ok(())
    }

    fn needs_to_handle(_linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        None
    }
}

impl StatsProvider for CpuAcct {
    type Stats = CpuUsage;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let mut stats = CpuUsage::default();
        Self::get_total_cpu_usage(cgroup_path, &mut stats)?;
        Self::get_per_core_usage(cgroup_path, &mut stats)?;

        Ok(stats)
    }
}

impl CpuAcct {
    fn get_total_cpu_usage(cgroup_path: &Path, stats: &mut CpuUsage) -> Result<()> {
        let stat_file_path = cgroup_path.join(CGROUP_CPUACCT_STAT);
        let stat_file_content = common::read_cgroup_file(&stat_file_path)?;

        // the first two entries of the file should look like this
        // user 746908
        // system 213896
        let parts: Vec<&str> = stat_file_content.split_whitespace().collect();

        if parts.len() < 4 {
            bail!(
                "{} contains less than the expected number of entries",
                stat_file_path.display()
            );
        }

        if !parts[0].eq("user") {
            bail!(
                "{} does not contain user mode cpu usage",
                stat_file_path.display()
            );
        }

        if !parts[2].eq("system") {
            bail!(
                "{} does not contain kernel mode cpu usage",
                stat_file_path.display()
            );
        }

        stats.usage_user = parts[1]
            .parse()
            .context("failed to parse user mode cpu usage")?;
        stats.usage_kernel = parts[3]
            .parse()
            .context("failed to parse kernel mode cpu usage")?;

        let total = common::read_cgroup_file(cgroup_path.join(CGROUP_CPUACCT_USAGE))?;
        stats.usage_total = total
            .trim()
            .parse()
            .context("failed to parse total cpu usage")?;

        Ok(())
    }

    fn get_per_core_usage(cgroup_path: &Path, stats: &mut CpuUsage) -> Result<()> {
        let all_content = common::read_cgroup_file(cgroup_path.join(CGROUP_CPUACCT_USAGE_ALL))?;
        // first line is header, skip it
        for entry in all_content.lines().skip(1) {
            let entry_parts: Vec<&str> = entry.split_ascii_whitespace().collect();
            if entry_parts.len() != 3 {
                continue;
            }

            stats.per_core_usage_user.push(
                entry_parts[1]
                    .parse()
                    .context("failed to parse per core user mode cpu usage")?,
            );
            stats.per_core_usage_kernel.push(
                entry_parts[2]
                    .parse()
                    .context("failed to parse per core kernel mode cpu usage")?,
            );
        }

        let percpu_content = common::read_cgroup_file(cgroup_path.join(CGROUP_CPUACCT_PERCPU))?;
        stats.per_core_usage_total = percpu_content
            .split_ascii_whitespace()
            .map(|v| v.parse())
            .collect::<Result<Vec<_>, _>>()
            .context("failed to parse per core cpu usage")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use nix::unistd::Pid;

    use super::*;
    use crate::cgroups::{common::CGROUP_PROCS, test::setup};

    #[test]
    fn test_add_task() {
        let (tmp, procs) = setup("test_cpuacct_apply", CGROUP_PROCS);
        let pid = Pid::from_raw(1000);

        CpuAcct::add_task(pid, &tmp).expect("apply cpuacct");

        let content = fs::read_to_string(&procs)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_PROCS));
        assert_eq!(content, "1000");
    }
}
