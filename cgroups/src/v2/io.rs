use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::{
    common::{self, ControllerOpt},
    stats::{self, BlkioDeviceStat, BlkioStats, StatsProvider},
};

use super::controller::Controller;
use oci_spec::runtime::LinuxBlockIo;

const CGROUP_BFQ_IO_WEIGHT: &str = "io.bfq.weight";
const CGROUP_IO_WEIGHT: &str = "io.weight";
const CGROUP_IO_STAT: &str = "io.stat";

pub struct Io {}

impl Controller for Io {
    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply io cgroup v2 config");
        if let Some(io) = &controller_opt.resources.block_io() {
            Self::apply(cgroup_root, io).context("failed to apply io resource restrictions")?;
        }
        Ok(())
    }
}

impl StatsProvider for Io {
    type Stats = BlkioStats;

    fn stats(cgroup_path: &Path) -> Result<Self::Stats> {
        let keyed_data = stats::parse_nested_keyed_data(&cgroup_path.join(CGROUP_IO_STAT))?;
        let mut service_bytes = Vec::with_capacity(keyed_data.len());
        let mut serviced = Vec::with_capacity(keyed_data.len());
        for entry in keyed_data {
            let (major, minor) = stats::parse_device_number(&entry.0)?;
            for value in &entry.1 {
                if value.starts_with("rbytes") {
                    service_bytes.push(BlkioDeviceStat {
                        major,
                        minor,
                        op_type: Some("read".to_owned()),
                        value: stats::parse_value(&value[7..])?,
                    });
                } else if value.starts_with("wbytes") {
                    service_bytes.push(BlkioDeviceStat {
                        major,
                        minor,
                        op_type: Some("write".to_owned()),
                        value: stats::parse_value(&value[7..])?,
                    });
                } else if value.starts_with("rios") {
                    serviced.push(BlkioDeviceStat {
                        major,
                        minor,
                        op_type: Some("read".to_owned()),
                        value: stats::parse_value(&value[5..])?,
                    });
                } else if value.starts_with("wios") {
                    serviced.push(BlkioDeviceStat {
                        major,
                        minor,
                        op_type: Some("write".to_owned()),
                        value: stats::parse_value(&value[5..])?,
                    });
                }
            }
        }

        let stats = BlkioStats {
            service_bytes,
            serviced,
            ..Default::default()
        };

        Ok(stats)
    }
}

impl Io {
    fn io_max_path(path: &Path) -> PathBuf {
        path.join("io.max")
    }

    // linux kernel doc: https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#io
    fn apply(root_path: &Path, blkio: &LinuxBlockIo) -> Result<()> {
        if let Some(weight_device) = blkio.weight_device().as_ref() {
            for wd in weight_device {
                common::write_cgroup_file(
                    root_path.join(CGROUP_BFQ_IO_WEIGHT),
                    &format!("{}:{} {}", wd.major(), wd.minor(), wd.weight().unwrap()),
                )?;
            }
        }
        if let Some(leaf_weight) = blkio.leaf_weight() {
            if leaf_weight > 0 {
                bail!("cannot set leaf_weight with cgroupv2");
            }
        }
        if let Some(io_weight) = blkio.weight() {
            if io_weight > 0 {
                common::write_cgroup_file(
                    root_path.join(CGROUP_IO_WEIGHT),
                    format!("{}", io_weight),
                )?;
            }
        }

        if let Some(throttle_read_bps_device) = blkio.throttle_read_bps_device().as_ref() {
            for trbd in throttle_read_bps_device {
                common::write_cgroup_file(
                    Self::io_max_path(root_path),
                    &format!("{}:{} rbps={}", trbd.major(), trbd.minor(), trbd.rate()),
                )?;
            }
        }

        if let Some(throttle_write_bps_device) = blkio.throttle_write_bps_device().as_ref() {
            for twbd in throttle_write_bps_device {
                common::write_cgroup_file(
                    Self::io_max_path(root_path),
                    format!("{}:{} wbps={}", twbd.major(), twbd.minor(), twbd.rate()),
                )?;
            }
        }

        if let Some(throttle_read_iops_device) = blkio.throttle_read_iops_device().as_ref() {
            for trid in throttle_read_iops_device {
                common::write_cgroup_file(
                    Self::io_max_path(root_path),
                    format!("{}:{} riops={}", trid.major(), trid.minor(), trid.rate()),
                )?;
            }
        }

        if let Some(throttle_write_iops_device) = blkio.throttle_write_iops_device().as_ref() {
            for twid in throttle_write_iops_device {
                common::write_cgroup_file(
                    Self::io_max_path(root_path),
                    format!("{}:{} wiops={}", twid.major(), twid.minor(), twid.rate()),
                )?;
            }
        }

        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture, setup};

    use oci_spec::runtime::{
        LinuxBlockIoBuilder, LinuxThrottleDeviceBuilder, LinuxWeightDeviceBuilder,
    };
    use std::fs;

    #[test]
    fn test_set_io_read_bps() {
        let (tmp, throttle) = setup("test_set_io_read_bps", "io.max");

        let blkio = LinuxBlockIoBuilder::default()
            .throttle_read_bps_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(8)
                .minor(0)
                .rate(102400u64)
                .build()
                .unwrap()])
            .build()
            .unwrap();

        Io::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle).unwrap_or_else(|_| panic!("read rbps content"));

        assert_eq!("8:0 rbps=102400", content);
    }

    #[test]
    fn test_set_io_write_bps() {
        let (tmp, throttle) = setup("test_set_io_write_bps", "io.max");

        let blkio = LinuxBlockIoBuilder::default()
            .throttle_write_bps_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(8)
                .minor(0)
                .rate(102400u64)
                .build()
                .unwrap()])
            .build()
            .unwrap();

        Io::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle).unwrap_or_else(|_| panic!("read rbps content"));

        assert_eq!("8:0 wbps=102400", content);
    }

    #[test]
    fn test_set_io_read_iops() {
        let (tmp, throttle) = setup("test_set_io_read_iops", "io.max");

        let blkio = LinuxBlockIoBuilder::default()
            .throttle_read_iops_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(8)
                .minor(0)
                .rate(102400u64)
                .build()
                .unwrap()])
            .build()
            .unwrap();

        Io::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle).unwrap_or_else(|_| panic!("read riops content"));

        assert_eq!("8:0 riops=102400", content);
    }

    #[test]
    fn test_set_io_write_iops() {
        let (tmp, throttle) = setup("test_set_io_write_iops", "io.max");

        let blkio = LinuxBlockIoBuilder::default()
            .throttle_write_iops_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(8)
                .minor(0)
                .rate(102400u64)
                .build()
                .unwrap()])
            .build()
            .unwrap();

        Io::apply(&tmp, &blkio).expect("apply blkio");
        let content = fs::read_to_string(throttle).unwrap_or_else(|_| panic!("read wiops content"));

        assert_eq!("8:0 wiops=102400", content);
    }

    #[test]
    fn test_set_ioweight_device() {
        let (tmp, throttle) = setup("test_set_io_weight_device", CGROUP_BFQ_IO_WEIGHT);
        let blkio = LinuxBlockIoBuilder::default()
            .weight_device(vec![LinuxWeightDeviceBuilder::default()
                .major(8)
                .minor(0)
                .weight(80u16)
                .leaf_weight(0u16)
                .build()
                .unwrap()])
            .build()
            .unwrap();

        Io::apply(&tmp, &blkio).expect("apply blkio");
        let content =
            fs::read_to_string(throttle).unwrap_or_else(|_| panic!("read bfq_io_weight content"));

        assert_eq!("8:0 80", content);
    }

    #[test]
    fn test_set_ioweight() {
        let (tmp, throttle) = setup("test_set_io_weight", CGROUP_IO_WEIGHT);
        let blkio = LinuxBlockIoBuilder::default()
            .weight(100u16)
            .build()
            .unwrap();

        Io::apply(&tmp, &blkio).expect("apply blkio");
        let content =
            fs::read_to_string(throttle).unwrap_or_else(|_| panic!("read bfq_io_weight content"));

        assert_eq!("100", content);
    }

    #[test]
    fn test_stat_io() {
        let tmp = create_temp_dir("test_stat_io").expect("create test directory");
        let stat_content = [
            "7:10 rbytes=18432 wbytes=16842 rios=12 wios=0 dbytes=0 dios=0",
            "7:9 rbytes=34629632 wbytes=274965 rios=1066 wios=319 dbytes=0 dios=0",
        ]
        .join("\n");
        set_fixture(&tmp, "io.stat", &stat_content).unwrap();

        let mut actual = Io::stats(&tmp).expect("get cgroup stats");
        let expected = BlkioStats {
            service_bytes: vec![
                BlkioDeviceStat {
                    major: 7,
                    minor: 9,
                    op_type: Some("read".to_owned()),
                    value: 34629632,
                },
                BlkioDeviceStat {
                    major: 7,
                    minor: 9,
                    op_type: Some("write".to_owned()),
                    value: 274965,
                },
                BlkioDeviceStat {
                    major: 7,
                    minor: 10,
                    op_type: Some("read".to_owned()),
                    value: 18432,
                },
                BlkioDeviceStat {
                    major: 7,
                    minor: 10,
                    op_type: Some("write".to_owned()),
                    value: 16842,
                },
            ],
            serviced: vec![
                BlkioDeviceStat {
                    major: 7,
                    minor: 9,
                    op_type: Some("read".to_owned()),
                    value: 1066,
                },
                BlkioDeviceStat {
                    major: 7,
                    minor: 9,
                    op_type: Some("write".to_owned()),
                    value: 319,
                },
                BlkioDeviceStat {
                    major: 7,
                    minor: 10,
                    op_type: Some("read".to_owned()),
                    value: 12,
                },
                BlkioDeviceStat {
                    major: 7,
                    minor: 10,
                    op_type: Some("write".to_owned()),
                    value: 0,
                },
            ],
            ..Default::default()
        };

        actual.service_bytes.sort();
        actual.serviced.sort();

        assert_eq!(actual, expected);
    }
}
