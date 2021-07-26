use anyhow::Result;
use std::path::Path;

pub trait StatsProvider {
    type Stats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats>;
}

#[derive(Debug)]
pub struct Stats {
    pub cpu: CpuStats,
    pub pids: PidStats,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            cpu: CpuStats::default(),
            pids: PidStats::default(),
        }
    }
}

#[derive(Debug)]
/// Reports the cpu statistics for a cgroup
pub struct CpuStats {
    pub usage: CpuUsage,
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

pub struct HugeTlbStats {}
