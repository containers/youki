use anyhow::Result;
use std::{collections::HashMap, fmt::Display, path::Path};

pub trait StatsProvider {
    type Stats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats>;
}

/// Reports the statistics for a cgroup
#[derive(Debug)]
pub struct Stats {
    /// Cpu statistics for the cgroup
    pub cpu: CpuStats,
    /// Pid statistics for the cgroup
    pub pids: PidStats,
    /// Hugetlb statistics for the cgroup
    pub hugetlb: HashMap<String, HugeTlbStats>,
    /// Blkio statistics for the cgroup
    pub blkio: BlkioStats,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            cpu: CpuStats::default(),
            pids: PidStats::default(),
            hugetlb: HashMap::new(),
            blkio: BlkioStats::default(),
        }
    }
}

/// Reports the cpu statistics for a cgroup
#[derive(Debug)]
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

/// Reports the cpu usage for a cgroup
#[derive(Debug, PartialEq, Eq)]
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

/// Reports the cpu throttling for a cgroup
#[derive(Debug, PartialEq, Eq)]
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

/// Reports pid stats for a cgroup
#[derive(Debug, PartialEq, Eq)]
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

/// Reports block io stats for a cgroup
#[derive(Debug, PartialEq, Eq)]
pub struct BlkioStats {
    // Number of bytes transfered to/from a device by the cgroup
    pub service_bytes: Vec<BlkioDeviceStat>,
    // Number of I/O operations performed on a device by the cgroup
    pub serviced: Vec<BlkioDeviceStat>,
    // Time in milliseconds that the cgroup had access to a device
    pub time: Vec<BlkioDeviceStat>,
    // Number of sectors transferred to/from a device by the cgroup
    pub sectors: Vec<BlkioDeviceStat>,
    // Total time between request dispatch and request completion
    pub service_time: Vec<BlkioDeviceStat>,
    // Total time spend waiting in the scheduler queues for service
    pub wait_time: Vec<BlkioDeviceStat>,
    // Number of requests queued for I/O operations
    pub queued: Vec<BlkioDeviceStat>,
    // Number of requests merged into requests for I/O operations
    pub merged: Vec<BlkioDeviceStat>,
}

impl Default for BlkioStats {
    fn default() -> Self {
        Self {
            service_bytes: Vec::new(),
            serviced: Vec::new(),
            time: Vec::new(),
            sectors: Vec::new(),
            service_time: Vec::new(),
            wait_time: Vec::new(),
            queued: Vec::new(),
            merged: Vec::new(),
        }
    }
}

/// Reports single value for a specific device
#[derive(Debug, PartialEq, Eq)]
pub struct BlkioDeviceStat {
    /// Major device number
    pub major: u64,
    /// Minor device number
    pub minor: u64,
    /// Operation type
    pub op_type: Option<String>,
    /// Stat value
    pub value: u64,
}

impl Display for BlkioDeviceStat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(op_type) = &self.op_type {
            write!(
                f,
                "{}:{} {} {}",
                self.major, self.minor, op_type, self.value
            )
        } else {
            write!(f, "{}:{} {}", self.major, self.minor, self.value)
        }
    }
}

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
