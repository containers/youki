use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use oci_spec::runtime::{
    LinuxBlockIo, LinuxBlockIoBuilder, LinuxBuilder, LinuxResourcesBuilder,
    LinuxThrottleDeviceBuilder, LinuxWeightDeviceBuilder, Spec, SpecBuilder,
};
use test_framework::{test_result, ConditionalTest, TestGroup, TestResult};

use crate::utils::{
    test_outside_container,
    test_utils::{check_container_created, CGROUP_ROOT},
};

// -----> README
// for this test to work for all parameters, the kernel needs to be compiled with CFQ IO schedular
// for some reason, Ubuntu and other distributions might come with a kernel compiled with mq-deadline
// schedular, which does not expose/support options such as blkio.weight, blkio.weight_device
// for these we can skip all the tests, or skip the corresponding tests
// the current implementation skips corresponding tests, so one can test atleast bps and iops
// device on such systems
// check https://superuser.com/questions/1449688/a-couple-of-blkio-cgroup-files-are-not-present-in-linux-kernel-5
// and https://github.com/opencontainers/runc/issues/140
// for more info on this

#[derive(Debug, Default)]
struct WeightDevice {
    major: i64,
    minor: i64,
    weight: Option<u16>,
    leaf_weight: Option<u16>,
}

#[derive(Debug, Default)]
struct ThrottleDevice {
    major: i64,
    minor: i64,
    rate: u64,
}

#[derive(Debug, Default)]
struct BlockIO {
    weight: u16,
    leaf_weight: u16,
    weight_devices: Vec<WeightDevice>,
    throttle_read_bps_devices: Vec<ThrottleDevice>,
    throttle_write_bps_devices: Vec<ThrottleDevice>,
    throttle_read_iops_devices: Vec<ThrottleDevice>,
    throttle_write_iops_devices: Vec<ThrottleDevice>,
}

fn can_run() -> bool {
    Path::new("/sys/fs/cgroup/blkio").exists()
}

fn supports_weight() -> bool {
    Path::new("/sys/fs/cgroup/blkio/blkio.weight").exists()
}

fn supports_weight_devices() -> bool {
    Path::new("/sys/fs/cgroup/blkio/blkio.weight_devices").exists()
}

// even though the mq-deadline does have these,we better check in case
// for some system these are absent
fn supports_throttle_bps() -> bool {
    Path::new("/sys/fs/cgroup/blkio/blkio.throttle.read_bps_device").exists()
}

fn supports_throttle_iops() -> bool {
    Path::new("/sys/fs/cgroup/blkio/blkio.throttle.read_iops_device").exists()
}

fn create_spec(cgroup_name: &str, block_io: LinuxBlockIo) -> Result<Spec> {
    let spec = SpecBuilder::default()
        .linux(
            LinuxBuilder::default()
                .cgroups_path(Path::new("/runtime-test").join(cgroup_name))
                .resources(
                    LinuxResourcesBuilder::default()
                        .block_io(block_io)
                        .build()
                        .context("failed to build resource spec")?,
                )
                .build()
                .context("failed to build linux spec")?,
        )
        .build()
        .context("failed to build spec")?;

    Ok(spec)
}

/// parses /sys/fs/cgroup/blkio and creates BlockIO struct
fn get_blkio_data(path: &str) -> Result<BlockIO> {
    let mut device = BlockIO::default();

    // we assume that if weight is present, leaf_weight will also be present
    if supports_weight() {
        // weight
        let weight_string = fs::read_to_string(format!("{}/blkio.weight", path))
            .context("error in reading block io weight")?;
        device.weight = weight_string
            .parse()
            .context("error in parsing block io weight")?;

        // leaf weight
        let leaf_weight_string = fs::read_to_string(format!("{}/blkio.leaf_weight", path))
            .context("error in reading block io leaf weight")?;
        device.leaf_weight = leaf_weight_string
            .parse()
            .context("error in parsing block io weight")?;
    }

    // weight devices section ------------
    // we assume if device_weight is supported, then device_leaf_weight is also supported
    if supports_weight_devices() {
        // device weight
        let device_weight_string = fs::read_to_string(format!("{}/blkio.weight_device", path))
            .context("error in reading block io weight device")?;
        let mut weight_devices = Vec::new();
        // format is  <major>:<minor>  <bytes_per_second>
        for line in device_weight_string.lines() {
            let (device_data, weight) = line
                .split_once(" ")
                .context("invalid weight device format")?;
            let (major_str, minor_str) = device_data
                .split_once(":")
                .context("invalid major-minor number format")?;
            weight_devices.push(WeightDevice {
                major: major_str.parse().context("error in parsing major number")?,
                minor: minor_str.parse().context("error in parsing minor number")?,
                weight: Some(weight.parse().context("error in parsing weight")?),
                leaf_weight: None,
            });
        }

        // device leaf weight

        let device_leaf_weight_string =
            fs::read_to_string(format!("{}/blkio.leaf_weight_device", path))
                .context("error in reading block io leaf weight device")?;

        for line in device_leaf_weight_string.lines() {
            let (device_data, weight) = line
                .split_once(" ")
                .context("invalid weight device format")?;
            let (major_str, minor_str) = device_data
                .split_once(":")
                .context("invalid major-minor number format")?;
            let leaf_weight: u16 = weight.parse().context("error in parsing leaf weight")?;
            let major: i64 = major_str.parse().context("error in parsing major number")?;
            let minor: i64 = minor_str.parse().context("error in parsing minor number")?;
            let mut found = false;
            for dev in &mut weight_devices {
                if dev.major == major && dev.minor == minor {
                    dev.leaf_weight = Some(leaf_weight);
                    found = true;
                }
            }
            if !found {
                weight_devices.push(WeightDevice {
                    major,
                    minor,
                    weight: None,
                    leaf_weight: Some(leaf_weight),
                });
            }
        }

        device.weight_devices = weight_devices;
    }

    // throttle devices section -----

    // we assume that if read_bps is supported, write_bps is also supported
    if supports_throttle_bps() {
        // throttle read bps
        let throttle_read_bps_string =
            fs::read_to_string(format!("{}/blkio.throttle.read_bps_device", path))
                .context("error in reading block io write bps device")?;
        let mut throttle_devices = Vec::new();
        for line in throttle_read_bps_string.lines() {
            let (device_data, rate) = line
                .split_once(" ")
                .context("invalid weight device format")?;
            let (major_str, minor_str) = device_data
                .split_once(":")
                .context("invalid major-minor number format")?;
            throttle_devices.push(ThrottleDevice {
                major: major_str.parse().context("error in parsing major number")?,
                minor: minor_str.parse().context("error in parsing minor number")?,
                rate: rate
                    .parse()
                    .context("error in parsing throttle read bps rate")?,
            });
        }
        device.throttle_read_bps_devices = throttle_devices;

        // throttle write bps

        let throttle_write_bps_string =
            fs::read_to_string(format!("{}/blkio.throttle.write_bps_device", path))
                .context("error in reading block io write bps device")?;
        let mut throttle_devices = Vec::new();
        for line in throttle_write_bps_string.lines() {
            let (device_data, rate) = line
                .split_once(" ")
                .context("invalid weight device format")?;
            let (major_str, minor_str) = device_data
                .split_once(":")
                .context("invalid major-minor number format")?;
            throttle_devices.push(ThrottleDevice {
                major: major_str.parse().context("error in parsing major number")?,
                minor: minor_str.parse().context("error in parsing minor number")?,
                rate: rate
                    .parse()
                    .context("error in parsing throttle write bps rate")?,
            });
        }
        device.throttle_write_bps_devices = throttle_devices;
    }

    // we assume that is read_iops is supported, write_iops is also supported
    if supports_throttle_iops() {
        // throttle read iops

        let throttle_read_iops_string =
            fs::read_to_string(format!("{}/blkio.throttle.read_iops_device", path))
                .context("error in reading block io read iops device")?;
        let mut throttle_devices = Vec::new();
        for line in throttle_read_iops_string.lines() {
            let (device_data, rate) = line
                .split_once(" ")
                .context("invalid throttle device format")?;
            let (major_str, minor_str) = device_data
                .split_once(":")
                .context("invalid major-minor number format")?;
            throttle_devices.push(ThrottleDevice {
                major: major_str.parse().context("error in parsing major number")?,
                minor: minor_str.parse().context("error in parsing minor number")?,
                rate: rate
                    .parse()
                    .context("error in parsing throttle read iops rate")?,
            });
        }
        device.throttle_read_iops_devices = throttle_devices;

        // throttle write iops

        let throttle_write_iops_string =
            fs::read_to_string(format!("{}/blkio.throttle.write_iops_device", path))
                .context("error in reading block io write iops device")?;
        let mut throttle_devices = Vec::new();
        for line in throttle_write_iops_string.lines() {
            let (device_data, rate) = line
                .split_once(" ")
                .context("invalid throttle device format")?;
            let (major_str, minor_str) = device_data
                .split_once(":")
                .context("invalid major-minor number format")?;
            throttle_devices.push(ThrottleDevice {
                major: major_str.parse().context("error in parsing major number")?,
                minor: minor_str.parse().context("error in parsing minor number")?,
                rate: rate
                    .parse()
                    .context("error in parsing throttle write iops rate")?,
            });
        }
        device.throttle_write_iops_devices = throttle_devices;
    }

    Ok(device)
}

/// validates the BlockIO structure parsed from /sys/fs/cgroup/blkio
/// with the spec
fn validate_block_io(cgroup_name: &str, spec: &Spec) -> Result<()> {
    let cgroup_path = PathBuf::from(CGROUP_ROOT)
        .join("blkio/runtime-test")
        .join(cgroup_name);
    let block_io = get_blkio_data(cgroup_path.to_str().unwrap())?;

    let resources = spec.linux().as_ref().unwrap().resources().as_ref().unwrap();
    let spec_block_io = resources.block_io().as_ref().unwrap();
    // weight ------
    if supports_weight() {
        if spec_block_io.weight().is_none() {
            return Err(anyhow::anyhow!("spec block io weight is none"));
        }
        if spec_block_io.weight().unwrap() != block_io.weight {
            return Err(anyhow::anyhow!(
                "block io weight is set incorrectly, expected {}, actual {}",
                spec_block_io.weight().unwrap(),
                block_io.weight,
            ));
        }
        if spec_block_io.leaf_weight().is_none() {
            return Err(anyhow::anyhow!("spec block io leaf weight is none"));
        }
        if spec_block_io.leaf_weight().unwrap() != block_io.leaf_weight {
            return Err(anyhow::anyhow!(
                "block io leaf weight is set incorrectly, expected {}, actual {}",
                spec_block_io.leaf_weight().unwrap(),
                block_io.leaf_weight,
            ));
        }
    }

    // weight devices ------
    if supports_weight_devices() {
        for spec_device in spec_block_io.weight_device().as_ref().unwrap() {
            let spec_major = spec_device.major();
            let spec_minor = spec_device.minor();
            let mut found = false;
            for device in &block_io.weight_devices {
                if device.major == spec_major && device.minor == spec_minor {
                    found = true;
                    if device.weight != spec_device.weight() {
                        return Err(anyhow::anyhow!(
                            "blkio weight is set incorrectly for device {}:{}, expected {:?}, found {:?}",
                            spec_major,
                            spec_minor,
                            spec_device.weight(),
                            device.weight
                        ));
                    }
                    if device.leaf_weight != spec_device.leaf_weight() {
                        return Err(anyhow::anyhow!(
                            "blkio leaf weight is set incorrectly for device {}:{}, expected {:?}, found {:?}",
                            spec_major,
                            spec_minor,
                            spec_device.leaf_weight(),
                            device.leaf_weight
                        ));
                    }
                    break;
                }
            }
            if !found {
                return Err(anyhow::anyhow!(
                    "blkio weight device {}:{} not found, exists in spec",
                    spec_major,
                    spec_minor
                ));
            }
        }
    }
    // throttle bps ------
    if supports_throttle_bps() {
        for spec_device in spec_block_io.throttle_read_bps_device().as_ref().unwrap() {
            let spec_major = spec_device.major();
            let spec_minor = spec_device.minor();
            let mut found = false;
            for device in &block_io.throttle_read_bps_devices {
                if device.major == spec_major && device.minor == spec_minor {
                    found = true;
                    if device.rate != spec_device.rate() {
                        return Err(anyhow::anyhow!(
                            "blkio throttle read bps rate is set incorrectly for device {}:{}, expected {}, found {}",
                            spec_major,
                            spec_minor,
                            spec_device.rate(),
                            device.rate
                        ));
                    }
                    break;
                }
            }
            if !found {
                return Err(anyhow::anyhow!(
                    "blkio throttle read bps device {}:{} not found, exists in spec",
                    spec_major,
                    spec_minor
                ));
            }
        }
        for spec_device in spec_block_io.throttle_write_bps_device().as_ref().unwrap() {
            let spec_major = spec_device.major();
            let spec_minor = spec_device.minor();
            let mut found = false;
            for device in &block_io.throttle_write_bps_devices {
                if device.major == spec_major && device.minor == spec_minor {
                    found = true;
                    if device.rate != spec_device.rate() {
                        return Err(anyhow::anyhow!(
                            "blkio throttle write bps rate is set incorrectly for device {}:{}, expected {}, found {}",
                            spec_major,
                            spec_minor,
                            spec_device.rate(),
                            device.rate
                        ));
                    }
                    break;
                }
            }
            if !found {
                return Err(anyhow::anyhow!(
                    "blkio throttle write bps device {}:{} not found, exists in spec",
                    spec_major,
                    spec_minor
                ));
            }
        }
    }

    // throttle iops ------
    if supports_throttle_iops() {
        for spec_device in spec_block_io.throttle_read_iops_device().as_ref().unwrap() {
            let spec_major = spec_device.major();
            let spec_minor = spec_device.minor();
            let mut found = false;
            for device in &block_io.throttle_read_iops_devices {
                if device.major == spec_major && device.minor == spec_minor {
                    found = true;
                    if device.rate != spec_device.rate() {
                        return Err(anyhow::anyhow!(
                        "blkio throttle read iops rate is set incorrectly for device {}:{}, expected {}, found {}",
                        spec_major,
                        spec_minor,
                        spec_device.rate(),
                        device.rate
                    ));
                    }
                    break;
                }
            }
            if !found {
                return Err(anyhow::anyhow!(
                    "blkio throttle read iops device {}:{} not found, exists in spec",
                    spec_major,
                    spec_minor
                ));
            }
        }

        for spec_device in spec_block_io.throttle_write_iops_device().as_ref().unwrap() {
            let spec_major = spec_device.major();
            let spec_minor = spec_device.minor();
            let mut found = false;
            for device in &block_io.throttle_write_iops_devices {
                if device.major == spec_major && device.minor == spec_minor {
                    found = true;
                    if device.rate != spec_device.rate() {
                        return Err(anyhow::anyhow!(
                        "blkio throttle write iops rate is set incorrectly for device {}:{}, expected {}, found {}",
                        spec_major,
                        spec_minor,
                        spec_device.rate(),
                        device.rate
                    ));
                    }
                    break;
                }
            }
            if !found {
                return Err(anyhow::anyhow!(
                    "blkio throttle write iops device {}:{} not found, exists in spec",
                    spec_major,
                    spec_minor
                ));
            }
        }
    }

    Ok(())
}

fn test_blkio(test_name: &str, rate: u64, empty: bool) -> TestResult {
    // these "magic" numbers are taken from the original tests
    // https://github.com/opencontainers/runtime-tools/blob/1684d131456a6bc99b8e96aa4a99783f21e58d79/validation/linux_cgroups_blkio/linux_cgroups_blkio.go#L13-L20
    let weight: u16 = 500;
    let leaf_weight: u16 = 300;
    let major: i64 = 8;
    let minor: i64 = 0;

    let mut block_io_builder = LinuxBlockIoBuilder::default();

    if !empty && supports_weight() {
        block_io_builder = block_io_builder.weight(weight).leaf_weight(leaf_weight)
    }
    if supports_weight_devices() {
        block_io_builder = block_io_builder.weight_device(vec![
            LinuxWeightDeviceBuilder::default()
                .major(major)
                .minor(minor)
                .weight(weight)
                .build()
                .unwrap(),
            LinuxWeightDeviceBuilder::default()
                .major(major)
                .minor(minor)
                .leaf_weight(leaf_weight)
                .build()
                .unwrap(),
        ])
    }
    if supports_throttle_bps() {
        block_io_builder = block_io_builder
            .throttle_read_bps_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(major)
                .minor(minor)
                .rate(rate)
                .build()
                .unwrap()])
            .throttle_write_bps_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(major)
                .minor(minor)
                .rate(rate)
                .build()
                .unwrap()])
    }
    if supports_throttle_iops() {
        block_io_builder = block_io_builder
            .throttle_read_iops_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(major)
                .minor(minor)
                .rate(rate)
                .build()
                .unwrap()])
            .throttle_write_iops_device(vec![LinuxThrottleDeviceBuilder::default()
                .major(major)
                .minor(minor)
                .rate(rate)
                .build()
                .unwrap()]);
    }
    let spec = create_spec(
        test_name,
        block_io_builder
            .build()
            .context("failed to build block io spec")
            .unwrap(),
    )
    .unwrap();

    test_outside_container(spec.clone(), &|data| {
        test_result!(check_container_created(&data));
        test_result!(validate_block_io(test_name, &spec));
        TestResult::Passed
    })
}

pub fn get_test_group<'a>() -> TestGroup<'a> {
    let mut test_group = TestGroup::new("cgroup_v1_blkio");
    let non_empty_100kb = ConditionalTest::new(
        "non_empty_100kb",
        Box::new(can_run),
        Box::new(|| test_blkio("non_empty_100kb", 102400, false)),
    );

    let non_empty_200kb = ConditionalTest::new(
        "non_empty_200kb",
        Box::new(can_run),
        Box::new(|| test_blkio("non_empty_200kb", 204800, false)),
    );
    let empty_100kb = ConditionalTest::new(
        "empty_100kb",
        Box::new(can_run),
        Box::new(|| test_blkio("empty_100kb", 102400, true)),
    );
    let empty_200kb = ConditionalTest::new(
        "empty_200kb",
        Box::new(can_run),
        Box::new(|| test_blkio("empty_200kb", 204800, true)),
    );

    test_group.add(vec![
        Box::new(non_empty_100kb),
        Box::new(non_empty_200kb),
        Box::new(empty_100kb),
        Box::new(empty_200kb),
    ]);
    test_group
}
