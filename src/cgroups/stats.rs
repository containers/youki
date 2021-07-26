use anyhow::Result;
use std::{collections::HashMap, path::Path};

pub trait StatsProvider {
    type Stats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats>;
}

#[derive(Debug)]
/// Reports the statistics for a cgroup
pub struct Stats {
    /// Cpu statistics for the cgroup
    pub cpu: CpuStats,
    /// Pid statistics for the cgroup
    pub pids: PidStats,
    /// Hugetlb statistics for the cgroup
    pub hugetlb: HashMap<String, HugeTlbStats>,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            cpu: CpuStats::default(),
            pids: PidStats::default(),
            hugetlb: HashMap::new(),
        }
    }
}

#[derive(Debug)]
/// Reports the cpu statistics for a cgroup
pub struct CpuStats {
    /// Cpu usage statistics for the cgroup
    pub usage: CpuUsage,
    /// Cpu Throttling statistics for the cgroup
    pub throttling: CpuThrottling,
}

impl Default for CpuStats {
    fn default() -> Self {
        Self {
            usage: CpuUsage::default(),
            throttling: CpuThrottling::default(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Reports the cpu usage for a cgroup
pub struct CpuUsage {
    /// Cpu time consumed by tasks in total
    pub usage_total: u64,
    /// Cpu time consumed by tasks in user mode
    pub usage_user: u64,
    /// Cpu time consumed by tasks in kernel mode
    pub usage_kernel: u64,
    /// Cpu time consumed by tasks itemized per core
    pub per_core_usage_total: Vec<u64>,
    /// Cpu time consumed by tasks in user mode itemized per core
    pub per_core_usage_user: Vec<u64>,
    /// Cpu time consumed by tasks in kernel mode itemized per core
    pub per_core_usage_kernel: Vec<u64>,
}

impl Default for CpuUsage {
    fn default() -> Self {
        Self {
            usage_total: 0,
            usage_user: 0,
            usage_kernel: 0,
            per_core_usage_total: Vec::new(),
            per_core_usage_user: Vec::new(),
            per_core_usage_kernel: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Reports the cpu throttling for a cgroup
pub struct CpuThrottling {
    /// Number of period intervals (as specified in cpu.cfs_period_us) that have elapsed
    pub periods: u64,
    /// Number of period intervals where tasks have been throttled because they exhausted their quota
    pub throttled_periods: u64,
    /// Total time duration for which tasks have been throttled
    pub throttled_time: u64,
}

impl Default for CpuThrottling {
    fn default() -> Self {
        Self {
            periods: 0,
            throttled_periods: 0,
            throttled_time: 0,
        }
    }
}

pub struct MemoryStats {}

#[derive(Debug, PartialEq, Eq)]
/// Reports pid stats for a cgroup
pub struct PidStats {
    /// Current number of active pids
    pub current: u64,
    /// Allowed number of active pids (0 means no limit)
    pub limit: u64,
}

impl Default for PidStats {
    fn default() -> Self {
        Self {
            current: 0,
            limit: 0,
        }
    }
}

pub struct BlkioStats {}

/// Reports hugetlb stats for a cgroup
#[derive(Debug, PartialEq, Eq)]
pub struct HugeTlbStats {
    /// Current usage in bytes
    pub usage: u64,
    /// Maximum recorded usage in bytes
    pub max_usage: u64,
    /// Number of allocation failures due to HugeTlb usage limit
    pub fail_count: u64,
}

impl Default for HugeTlbStats {
    fn default() -> Self {
        Self {
            usage: 0,
            max_usage: 0,
            fail_count: 0,
        }
    }
}
