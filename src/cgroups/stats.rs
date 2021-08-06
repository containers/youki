use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{collections::HashMap, fmt::Display, fs, path::Path};

use super::common;

pub trait StatsProvider {
    type Stats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats>;
}

/// Reports the statistics for a cgroup
#[derive(Debug, Serialize)]
pub struct Stats {
    /// Cpu statistics for the cgroup
    pub cpu: CpuStats,
    /// Pid statistics for the cgroup
    pub pids: PidStats,
    /// Hugetlb statistics for the cgroup
    pub hugetlb: HashMap<String, HugeTlbStats>,
    /// Blkio statistics for the cgroup
    pub blkio: BlkioStats,
    /// Memory statistics for the cgroup
    pub memory: MemoryStats,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            cpu: CpuStats::default(),
            pids: PidStats::default(),
            hugetlb: HashMap::new(),
            blkio: BlkioStats::default(),
            memory: MemoryStats::default(),
        }
    }
}

/// Reports the cpu statistics for a cgroup
#[derive(Debug, Serialize)]
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
#[derive(Debug, PartialEq, Eq, Serialize)]
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
#[derive(Debug, PartialEq, Eq, Serialize)]
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

/// Reports memory stats for a cgroup
#[derive(Debug, Serialize)]
pub struct MemoryStats {
    /// Usage of memory
    pub memory: MemoryData,
    /// Usage of memory and swap
    pub memswap: MemoryData,
    /// Usage of kernel memory
    pub kernel: MemoryData,
    /// Usage of kernel tcp memory
    pub kernel_tcp: MemoryData,
    /// Page cache in bytes
    pub cache: u64,
    /// Returns true if hierarchical accounting is enabled
    pub hierarchy: bool,
    /// Various memory statistics
    pub stats: HashMap<String, u64>,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            memory: MemoryData::default(),
            memswap: MemoryData::default(),
            kernel: MemoryData::default(),
            kernel_tcp: MemoryData::default(),
            cache: 0,
            hierarchy: false,
            stats: HashMap::default(),
        }
    }
}

/// Reports memory stats for one type of memory
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MemoryData {
    /// Usage in bytes
    pub usage: u64,
    /// Maximum recorded usage in bytes
    pub max_usage: u64,
    /// Number of times memory usage hit limits
    pub fail_count: u64,
    /// Memory usage limit
    pub limit: u64,
}

impl Default for MemoryData {
    fn default() -> Self {
        Self {
            usage: 0,
            max_usage: 0,
            fail_count: 0,
            limit: 0,
        }
    }
}

/// Reports pid stats for a cgroup
#[derive(Debug, PartialEq, Eq, Serialize)]
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
#[derive(Debug, PartialEq, Eq, Serialize)]
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

/// Reports single stat value for a specific device
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
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
#[derive(Debug, PartialEq, Eq, Serialize)]
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

/// Reports which hugepage sizes are supported by the system
pub fn supported_page_sizes() -> Result<Vec<String>> {
    let mut sizes = Vec::new();
    for hugetlb_entry in fs::read_dir("/sys/kernel/mm/hugepages")? {
        let hugetlb_entry = hugetlb_entry?;
        if !hugetlb_entry.path().is_dir() {
            continue;
        }

        let dir_name = hugetlb_entry.file_name();
        let dir_name = dir_name.to_str().unwrap();

        sizes.push(extract_page_size(dir_name)?);
    }

    Ok(sizes)
}

fn extract_page_size(dir_name: &str) -> Result<String> {
    if let Some(name_stripped) = dir_name.strip_prefix("hugepages-") {
        if let Some(size) = name_stripped.strip_suffix("kB") {
            let size: u64 = parse_value(size)?;

            let size_moniker = if size >= (1 << 20) {
                (size >> 20).to_string() + "GB"
            } else if size >= (1 << 10) {
                (size >> 10).to_string() + "MB"
            } else {
                size.to_string() + "KB"
            };

            return Ok(size_moniker);
        }
    }

    bail!("failed to determine page size from {}", dir_name);
}

/// Parses this string slice into an u64
/// # Example
/// ```
/// use youki::cgroups::stats::parse_value;
///
/// let value = parse_value("32").unwrap();
/// assert_eq!(value, 32);
/// ```
pub fn parse_value(value: &str) -> Result<u64> {
    value
        .parse()
        .with_context(|| format!("failed to parse {}", value))
}

/// Parses a single valued file to an u64
/// # Example
/// ```no_run
/// use std::path::Path;
/// use youki::cgroups::stats::parse_single_value;
///
/// let value = parse_single_value(&Path::new("memory.current")).unwrap();
/// assert_eq!(value, 32);
/// ```
pub fn parse_single_value(file_path: &Path) -> Result<u64> {
    let value = common::read_cgroup_file(file_path)?;
    let value = value.trim();
    if value == "max" {
        return Ok(u64::MAX);
    }

    value.parse().with_context(|| {
        format!(
            "failed to parse value {} from {}",
            value,
            file_path.display()
        )
    })
}

/// Parses a file that is structed according to the flat keyed format
pub fn parse_flat_keyed_data(file_path: &Path) -> Result<HashMap<String, u64>> {
    let mut stats = HashMap::new();
    let keyed_data = common::read_cgroup_file(file_path)?;
    for entry in keyed_data.lines() {
        let entry_fields: Vec<&str> = entry.split_ascii_whitespace().collect();
        if entry_fields.len() != 2 {
            bail!(
                "flat keyed data at {} contains entries that do not conform to 'key value'",
                &file_path.display()
            );
        }

        stats.insert(
            entry_fields[0].to_owned(),
            entry_fields[1].parse().with_context(|| {
                format!(
                    "failed to parse value {} from {}",
                    entry_fields[0],
                    file_path.display()
                )
            })?,
        );
    }

    Ok(stats)
}

/// Parses a file that is structed according to the nested keyed format
pub fn parse_nested_keyed_data(file_path: &Path) -> Result<HashMap<String, Vec<String>>> {
    let mut stats: HashMap<String, Vec<String>> = HashMap::new();
    let keyed_data = common::read_cgroup_file(file_path)?;
    for entry in keyed_data.lines() {
        let entry_fields: Vec<&str> = entry.split_ascii_whitespace().collect();
        if entry_fields.len() < 2 || !entry_fields[1..].iter().all(|p| p.contains('=')) {
            bail!("nested key data at {} contains entries that do not conform to the nested key format", file_path.display());
        }

        stats.insert(
            entry_fields[0].to_owned(),
            entry_fields[1..]
                .iter()
                .copied()
                .map(|p| p.to_owned())
                .collect(),
        );
    }

    Ok(stats)
}

/// Parses a file that is structed according to the nested keyed format
/// # Example
/// ```
/// use youki::cgroups::stats::parse_device_number;
///
/// let (major, minor) = parse_device_number("8:0").unwrap();
/// assert_eq!((major, minor), (8, 0));
/// ```
pub fn parse_device_number(device: &str) -> Result<(u64, u64)> {
    let numbers: Vec<&str> = device.split_terminator(':').collect();
    if numbers.len() != 2 {
        bail!("failed to parse device number {}", device);
    }

    Ok((numbers[0].parse()?, numbers[1].parse()?))
}

/// Returns cgroup pid statistics
pub fn pid_stats(cgroup_path: &Path) -> Result<PidStats> {
    let mut stats = PidStats::default();

    let current = common::read_cgroup_file(cgroup_path.join("pids.current"))?;
    stats.current = current
        .trim()
        .parse()
        .context("failed to parse current pids")?;

    let limit =
        common::read_cgroup_file(cgroup_path.join("pids.max")).map(|l| l.trim().to_owned())?;
    if limit != "max" {
        stats.limit = limit.parse().context("failed to parse pids limit")?;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use crate::{cgroups::test::set_fixture, utils::create_temp_dir};

    use super::*;

    #[test]
    fn test_supported_page_sizes_gigabyte() {
        let page_size = extract_page_size("hugepages-1048576kB").unwrap();
        assert_eq!(page_size, "1GB");
    }

    #[test]
    fn test_supported_page_sizes_megabyte() {
        let page_size = extract_page_size("hugepages-2048kB").unwrap();
        assert_eq!(page_size, "2MB");
    }

    #[test]
    fn test_supported_page_sizes_kilobyte() {
        let page_size = extract_page_size("hugepages-512kB").unwrap();
        assert_eq!(page_size, "512KB");
    }

    #[test]
    fn test_parse_single_value_valid() {
        let tmp = create_temp_dir("test_parse_single_value_valid").unwrap();
        let file_path = set_fixture(&tmp, "single_valued_file", "1200\n").unwrap();

        let value = parse_single_value(&file_path).unwrap();
        assert_eq!(value, 1200);
    }

    #[test]
    fn test_parse_single_value_invalid_number() {
        let tmp = create_temp_dir("test_parse_single_value_invalid_number").unwrap();
        let file_path = set_fixture(&tmp, "single_invalid_file", "noop\n").unwrap();

        let value = parse_single_value(&file_path);
        assert!(value.is_err());
    }

    #[test]
    fn test_parse_single_value_multiple_entries() {
        let tmp = create_temp_dir("test_parse_single_value_multiple_entries").unwrap();
        let file_path = set_fixture(&tmp, "multi_valued_file", "1200\n1400\n1600").unwrap();

        let value = parse_single_value(&file_path);
        assert!(value.is_err());
    }

    #[test]
    fn test_parse_flat_keyed_data() {
        let tmp = create_temp_dir("test_parse_flat_keyed_data").unwrap();
        let file_content = ["key1 1", "key2 2", "key3 3"].join("\n");
        let file_path = set_fixture(&tmp, "flat_keyed_data", &file_content).unwrap();

        let actual = parse_flat_keyed_data(&file_path).unwrap();
        let mut expected = HashMap::with_capacity(3);
        expected.insert("key1".to_owned(), 1);
        expected.insert("key2".to_owned(), 2);
        expected.insert("key3".to_owned(), 3);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_flat_keyed_data_with_characters() {
        let tmp = create_temp_dir("test_parse_flat_keyed_data_with_characters").unwrap();
        let file_content = ["key1 1", "key2 a", "key3 b"].join("\n");
        let file_path = set_fixture(&tmp, "flat_keyed_data", &file_content).unwrap();

        let result = parse_flat_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_space_separated_as_flat_keyed_data() {
        let tmp = create_temp_dir("test_parse_space_separated_as_flat_keyed_data").unwrap();
        let file_content = ["key1", "key2", "key3", "key4"].join(" ");
        let file_path = set_fixture(&tmp, "space_separated", &file_content).unwrap();

        let result = parse_flat_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_newline_separated_as_flat_keyed_data() {
        let tmp = create_temp_dir("test_parse_newline_separated_as_flat_keyed_data").unwrap();
        let file_content = ["key1", "key2", "key3", "key4"].join("\n");
        let file_path = set_fixture(&tmp, "newline_separated", &file_content).unwrap();

        let result = parse_flat_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_nested_keyed_data_as_flat_keyed_data() {
        let tmp = create_temp_dir("test_parse_nested_keyed_data_as_flat_keyed_data").unwrap();
        let file_content = [
            "key1 subkey1=value1 subkey2=value2 subkey3=value3",
            "key2 subkey1=value1 subkey2=value2 subkey3=value3",
            "key3 subkey1=value1 subkey2=value2 subkey3=value3",
        ]
        .join("\n");
        let file_path = set_fixture(&tmp, "nested_keyed_data", &file_content).unwrap();

        let result = parse_flat_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_nested_keyed_data() {
        let tmp = create_temp_dir("test_parse_nested_keyed_data").unwrap();
        let file_content = [
            "key1 subkey1=value1 subkey2=value2 subkey3=value3",
            "key2 subkey1=value1 subkey2=value2 subkey3=value3",
            "key3 subkey1=value1 subkey2=value2 subkey3=value3",
        ]
        .join("\n");
        let file_path = set_fixture(&tmp, "nested_keyed_data", &file_content).unwrap();

        let actual = parse_nested_keyed_data(&file_path).unwrap();
        let mut expected = HashMap::with_capacity(3);
        expected.insert(
            "key1".to_owned(),
            vec![
                "subkey1=value1".to_owned(),
                "subkey2=value2".to_owned(),
                "subkey3=value3".to_owned(),
            ],
        );
        expected.insert(
            "key2".to_owned(),
            vec![
                "subkey1=value1".to_owned(),
                "subkey2=value2".to_owned(),
                "subkey3=value3".to_owned(),
            ],
        );
        expected.insert(
            "key3".to_owned(),
            vec![
                "subkey1=value1".to_owned(),
                "subkey2=value2".to_owned(),
                "subkey3=value3".to_owned(),
            ],
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_space_separated_as_nested_keyed_data() {
        let tmp = create_temp_dir("test_parse_newline_separated_as_nested_keyed_data").unwrap();
        let file_content = ["key1", "key2", "key3", "key4"].join(" ");
        let file_path = set_fixture(&tmp, "space_separated", &file_content).unwrap();

        let result = parse_nested_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_newline_separated_as_nested_keyed_data() {
        let tmp = create_temp_dir("test_parse_newline_separated_as_nested_keyed_data").unwrap();
        let file_content = ["key1", "key2", "key3", "key4"].join("\n");
        let file_path = set_fixture(&tmp, "newline_separated", &file_content).unwrap();

        let result = parse_nested_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_flat_keyed_as_nested_keyed_data() {
        let tmp = create_temp_dir("test_parse_newline_separated_as_nested_keyed_data").unwrap();
        let file_content = ["key1 1", "key2 2", "key3 3"].join("\n");
        let file_path = set_fixture(&tmp, "newline_separated", &file_content).unwrap();

        let result = parse_nested_keyed_data(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_device_number() {
        let (major, minor) = parse_device_number("8:0").unwrap();
        assert_eq!((major, minor), (8, 0));
    }

    #[test]
    fn test_parse_invalid_device_number() {
        let result = parse_device_number("a:b");
        assert!(result.is_err());
    }
}
