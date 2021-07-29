use std::path::Path;

use crate::cgroups::{
    common,
    stats::{BlkioDeviceStat, BlkioStats, StatsProvider},
    v1::Controller,
};
use anyhow::{bail, Context, Result};
use oci_spec::{LinuxBlockIo, LinuxResources};

// Throttling/upper limit policy
// ---------------------------------------
// Upper limit on the number of read operations a device can perform specified in bytes
// Format: Major:Minor Bytes
const BLKIO_THROTTLE_READ_BPS: &str = "blkio.throttle.read_bps_device";
// Upper limit on the number of write operations a device can perform specified in bytes
// Format: Major:Minor Bytes
const BLKIO_THROTTLE_WRITE_BPS: &str = "blkio.throttle.write_bps_device";
// Upper limit on the number of read operations a device can perform specified in operations per second
// Format: Major:Minor Ops
const BLKIO_THROTTLE_READ_IOPS: &str = "blkio.throttle.read_iops_device";
// Upper limit on the number of write operations a device can perform specified in operations per second
// Format: Major:Minor Ops
const BLKIO_THROTTLE_WRITE_IOPS: &str = "blkio.throttle.write_iops_device";
// Number of I/O operations performed on a device by the cgroup
// Format: Major:Minor Type Ops
const BLKIO_THROTTLE_IO_SERVICED: &str = "blkio.throttle.io_serviced";
// Number of bytes transfered to/from a device by the cgroup
// Format: Major:Minor Type Bytes
const BLKIO_THROTTLE_IO_SERVICE_BYTES: &str = "blkio.throttle.io_service_bytes";

// Proportional weight division policy
// ---------------------------------------
// Specifies the relative proportion of block I/O access available to the cgroup
// Format: weight (weight can range from 100 to 1000)
const BLKIO_WEIGHT: &str = "blkio.weight";
// Specifies the relative proportion of block I/O access for specific devices available
// to the cgroup. This overrides the the blkio.weight value for the specified device
// Format: Major:Minor weight (weight can range from 100 to 1000)
#[allow(dead_code)]
const BLKIO_WEIGHT_DEVICE: &str = "blkio.weight_device";

// Common parameters which may be used for either policy but seem to be used only for
// proportional weight division policy in practice
// ---------------------------------------
// Time in milliseconds that the cgroup had access to a device
// Format: Major:Minor Time(ms)
const BLKIO_TIME: &str = "blkio.time_recursive";
// Number of sectors transferred to/from a device by the cgroup
// Format: Major:Minor Sectors
const BLKIO_SECTORS: &str = "blkio.sectors_recursive";
// Number of bytes transfered to/from a device by the cgroup
/// Format: Major:Minor Type Bytes
const BLKIO_IO_SERVICE_BYTES: &str = "blkio.io_service_bytes_recursive";
// Number of I/O operations performed on a device by the cgroup
// Format: Major:Minor Type Ops
const BLKIO_IO_SERVICED: &str = "blkio.io_serviced_recursive";
// Total time between request dispatch and request completion
//// Format: Major:Minor Type Time(ns)
const BLKIO_IO_SERVICE_TIME: &str = "blkio.io_service_time_recursive";
// Total time spend waiting in the scheduler queues for service
// Format: Major:Minor Type Time(ns)
const BLKIO_WAIT_TIME: &str = "blkio.io_wait_time_recursive";
// Number of requests queued for I/O operations
// Format: Requests Type
const BLKIO_QUEUED: &str = "blkio.io_queued_recursive";
// Number of requests merged into requests for I/O operations
// Format: Requests Type
const BLKIO_MERGED: &str = "blkio.io_merged_recursive";

pub struct Blkio {}

impl Controller for Blkio {
    type Resource = LinuxBlockIo;

    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply blkio cgroup config");

        if let Some(blkio) = Self::needs_to_handle(linux_resources) {
            Self::apply(cgroup_root, blkio)?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(blkio) = &linux_resources.block_io {
            return Some(blkio);
        }

        None
    }
}

impl StatsProvider for Blkio {
    type Stats = BlkioStats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        if cgroup_path.join(BLKIO_WEIGHT).exists() {
            return Self::get_weight_division_policy_stats(cgroup_path);
        }

        Self::get_throttling_policy_stats(cgroup_path)
    }
}

impl Blkio {
    fn apply(root_path: &Path, blkio: &LinuxBlockIo) -> Result<()> {
        for trbd in &blkio.blkio_throttle_read_bps_device {
            common::write_cgroup_file_str(
                &root_path.join(BLKIO_THROTTLE_READ_BPS),
                &format!("{}:{} {}", trbd.major, trbd.minor, trbd.rate),
            )?;
        }

        for twbd in &blkio.blkio_throttle_write_bps_device {
            common::write_cgroup_file_str(
                &root_path.join(BLKIO_THROTTLE_WRITE_BPS),
                &format!("{}:{} {}", twbd.major, twbd.minor, twbd.rate),
            )?;
        }

        for trid in &blkio.blkio_throttle_read_iops_device {
            common::write_cgroup_file_str(
                &root_path.join(BLKIO_THROTTLE_READ_IOPS),
                &format!("{}:{} {}", trid.major, trid.minor, trid.rate),
            )?;
        }

        for twid in &blkio.blkio_throttle_write_iops_device {
            common::write_cgroup_file_str(
                &root_path.join(BLKIO_THROTTLE_WRITE_IOPS),
                &format!("{}:{} {}", twid.major, twid.minor, twid.rate),
            )?;
        }

        Ok(())
    }

    fn get_throttling_policy_stats(cgroup_path: &Path) -> Result<BlkioStats> {
        let stats = BlkioStats {
            service_bytes: Self::parse_blkio_file(
                &cgroup_path.join(BLKIO_THROTTLE_IO_SERVICE_BYTES),
            )?,
            serviced: Self::parse_blkio_file(&cgroup_path.join(BLKIO_THROTTLE_IO_SERVICED))?,
            ..Default::default()
        };

        Ok(stats)
    }

    fn get_weight_division_policy_stats(cgroup_path: &Path) -> Result<BlkioStats> {
        let stats = BlkioStats {
            time: Self::parse_blkio_file(&cgroup_path.join(BLKIO_TIME))?,
            sectors: Self::parse_blkio_file(&cgroup_path.join(BLKIO_SECTORS))?,
            service_bytes: Self::parse_blkio_file(&cgroup_path.join(BLKIO_IO_SERVICE_BYTES))?,
            serviced: Self::parse_blkio_file(&cgroup_path.join(BLKIO_IO_SERVICED))?,
            service_time: Self::parse_blkio_file(&cgroup_path.join(BLKIO_IO_SERVICE_TIME))?,
            wait_time: Self::parse_blkio_file(&cgroup_path.join(BLKIO_WAIT_TIME))?,
            queued: Self::parse_blkio_file(&cgroup_path.join(BLKIO_QUEUED))?,
            merged: Self::parse_blkio_file(&cgroup_path.join(BLKIO_MERGED))?,
        };

        Ok(stats)
    }

    fn parse_blkio_file(blkio_file: &Path) -> Result<Vec<BlkioDeviceStat>> {
        let content = common::read_cgroup_file(blkio_file)?;
        let mut stats = Vec::new();
        for entry in content.lines() {
            let entry_fields: Vec<&str> = entry.split_ascii_whitespace().collect();
            if entry_fields.len() <= 2 {
                continue;
            }

            let (major, minor) = Self::parse_device_number(entry_fields[0])?;
            let op_type = if entry_fields.len() == 3 {
                Some(entry_fields[1].to_owned())
            } else {
                None
            };
            let value = if entry_fields.len() == 3 {
                entry_fields[2].parse().with_context(|| {
                    format!(
                        "failed to parse device value {} in {}",
                        entry_fields[2],
                        blkio_file.display()
                    )
                })?
            } else {
                entry_fields[1].parse().with_context(|| {
                    format!(
                        "failed to parse device value {} in {}",
                        entry_fields[1],
                        blkio_file.display()
                    )
                })?
            };

            let stat = BlkioDeviceStat {
                major,
                minor,
                op_type,
                value,
            };

            stats.push(stat);
        }

        Ok(stats)
    }

    fn parse_device_number(entry: &str) -> Result<(u64, u64)> {
        let numbers: Vec<&str> = entry.split_terminator(':').collect();
        if numbers.len() != 2 {
            bail!("failed to parse device number {}", entry);
        }

        Ok((numbers[0].parse()?, numbers[1].parse()?))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{
        cgroups::test::{set_fixture, setup},
        utils::create_temp_dir,
    };
    use anyhow::Result;
    use oci_spec::{LinuxBlockIo, LinuxThrottleDevice};

    struct BlockIoBuilder {
        block_io: LinuxBlockIo,
    }

    impl BlockIoBuilder {
        fn new() -> Self {
            let block_io = LinuxBlockIo {
                blkio_weight: Some(0),
                blkio_leaf_weight: Some(0),
                blkio_weight_device: vec![],
                blkio_throttle_read_bps_device: vec![],
                blkio_throttle_write_bps_device: vec![],
                blkio_throttle_read_iops_device: vec![],
                blkio_throttle_write_iops_device: vec![],
            };

            Self { block_io }
        }

        fn with_read_bps(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_read_bps_device = throttle;
            self
        }

        fn with_write_bps(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_write_bps_device = throttle;
            self
        }

        fn with_read_iops(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_read_iops_device = throttle;
            self
        }

        fn with_write_iops(mut self, throttle: Vec<LinuxThrottleDevice>) -> Self {
            self.block_io.blkio_throttle_write_iops_device = throttle;
            self
        }

        fn build(self) -> LinuxBlockIo {
            self.block_io
        }
    }

    #[test]
    fn test_set_blkio_read_bps() {
        let (tmp, throttle) = setup("test_set_blkio_read_bps", BLKIO_THROTTLE_READ_BPS);

        let blkio = BlockIoBuilder::new()
            .with_read_bps(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        Blkio::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", BLKIO_THROTTLE_READ_BPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_set_blkio_write_bps() {
        let (tmp, throttle) = setup("test_set_blkio_write_bps", BLKIO_THROTTLE_WRITE_BPS);

        let blkio = BlockIoBuilder::new()
            .with_write_bps(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        Blkio::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", BLKIO_THROTTLE_WRITE_BPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_set_blkio_read_iops() {
        let (tmp, throttle) = setup("test_set_blkio_read_iops", BLKIO_THROTTLE_READ_IOPS);

        let blkio = BlockIoBuilder::new()
            .with_read_iops(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        Blkio::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", BLKIO_THROTTLE_READ_IOPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_set_blkio_write_iops() {
        let (tmp, throttle) = setup("test_set_blkio_write_iops", BLKIO_THROTTLE_WRITE_IOPS);

        let blkio = BlockIoBuilder::new()
            .with_write_iops(vec![LinuxThrottleDevice {
                major: 8,
                minor: 0,
                rate: 102400,
            }])
            .build();

        Blkio::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle)
            .unwrap_or_else(|_| panic!("read {} content", BLKIO_THROTTLE_WRITE_IOPS));

        assert_eq!("8:0 102400", content);
    }

    #[test]
    fn test_stat_throttling_policy() -> Result<()> {
        let tmp = create_temp_dir("test_stat_throttling_policy").expect("create test directory");
        let content = &[
            "8:0 Read 20",
            "8:0 Write 20",
            "8:0 Sync 20",
            "8:0 Async 20",
            "8:0 Discard 20",
            "8:0 Total 20",
            "Total 0",
        ]
        .join("\n");
        set_fixture(&tmp, BLKIO_THROTTLE_IO_SERVICE_BYTES, &content).unwrap();
        set_fixture(&tmp, BLKIO_THROTTLE_IO_SERVICED, &content).unwrap();

        let actual = Blkio::stats(&tmp).expect("get cgroup stats");
        let mut expected = BlkioStats::default();
        let devices: Vec<BlkioDeviceStat> = ["Read", "Write", "Sync", "Async", "Discard", "Total"]
            .iter()
            .copied()
            .map(|op| BlkioDeviceStat {
                major: 8,
                minor: 0,
                op_type: Some(op.to_owned()),
                value: 20,
            })
            .collect();

        expected.service_bytes = devices.clone();
        expected.serviced = devices;

        assert_eq!(expected, actual);
        Ok(())
    }
}
