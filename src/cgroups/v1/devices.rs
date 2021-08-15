use std::path::Path;
use std::fs::File;

use anyhow::Result;
use async_trait::async_trait;
use rio::Rio;

use crate::cgroups::common;
use crate::{cgroups::v1::Controller, rootfs::default_devices};
use oci_spec::{LinuxDeviceCgroup, LinuxDeviceType, LinuxResources};

pub struct Devices {}

#[async_trait]
impl Controller for Devices {
    type Resource = ();

    async fn apply(ring: &Rio, linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Devices cgroup config");
        let allowed_file = common::open_cgroup_file(cgroup_root.join("devices.allow"))?;
        let denied_file = common::open_cgroup_file(cgroup_root.join("devices.deny"))?;

        for d in &linux_resources.devices {
            Self::apply_device(ring, d, &allowed_file, &denied_file).await?;
        }

        for d in [
            default_devices().iter().map(|d| d.into()).collect(),
            Self::default_allow_devices(),
        ]
        .concat()
        {
            Self::apply_device(ring, &d, &allowed_file, &denied_file).await?;
        }

        Ok(())
    }

    // always needs to be called due to default devices
    fn needs_to_handle(_linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        Some(&())
    }
}

impl Devices {
    async fn apply_device(ring: &Rio, device: &LinuxDeviceCgroup, allowed_file: &File, denied_file: &File) -> Result<()> {
        let file = if device.allow { &denied_file } else { &allowed_file };
        common::async_write_cgroup_file_str(ring, file, &device.to_string()).await?;
        Ok(())
    }

    fn default_allow_devices() -> Vec<LinuxDeviceCgroup> {
        vec![
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: None,
                minor: None,
                access: "m".to_string(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::B,
                major: None,
                minor: None,
                access: "m".to_string(),
            },
            // /dev/console
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(5),
                minor: Some(1),
                access: "rwm".to_string(),
            },
            // /dev/pts
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(136),
                minor: None,
                access: "rwm".to_string(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(5),
                minor: Some(2),
                access: "rwm".to_string(),
            },
            // tun/tap
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(10),
                minor: Some(200),
                access: "rwm".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::{set_fixture, aw};
    use crate::utils::create_temp_dir;
    use oci_spec::{LinuxDeviceCgroup, LinuxDeviceType};
    use std::fs::read_to_string;

    #[test]
    fn test_set_default_devices() {
        let tmp =
            create_temp_dir("test_set_default_devices").expect("create temp directory for test");

        Devices::default_allow_devices().iter().for_each(|d| {
            // NOTE: We reset the fixtures every iteration because files aren't appended
            // so what happens in the tests is you get strange overwrites which can contain
            // remaining bytes from the last iteration. Resetting the files more appropriately
            // mocks the behavior of cgroup files.
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");
            let ring = rio::new().expect("start io_uring");
            let allowed_file = common::open_cgroup_file(tmp.join("devices.allow")).expect("open allowed devices");
            let denied_file = common::open_cgroup_file(tmp.join("devices.deny")).expect("open denied devices");

            aw!(Devices::apply_device(&ring, &d, &allowed_file, &denied_file)).expect("Apply default device");
            println!("Device: {}", d.to_string());
            if d.allow {
                let allowed_content =
                    read_to_string(tmp.join("devices.allow")).expect("read to string");
                assert_eq!(allowed_content, d.to_string());
            } else {
                let denied_content =
                    read_to_string(tmp.join("devices.deny")).expect("read to string");
                assert_eq!(denied_content, d.to_string());
            }
        });
    }

    #[test]
    fn test_set_mock_devices() {
        let tmp = create_temp_dir("test_set_mock_devices").expect("create temp directory for test");
        let ring = rio::new().expect("start io_uring");
        [
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::C,
                major: Some(10),
                minor: None,
                access: "rwm".to_string(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::A,
                major: None,
                minor: Some(200),
                access: "rwm".to_string(),
            },
            LinuxDeviceCgroup {
                allow: false,
                typ: LinuxDeviceType::P,
                major: Some(10),
                minor: Some(200),
                access: "m".to_string(),
            },
            LinuxDeviceCgroup {
                allow: false,
                typ: LinuxDeviceType::U,
                major: None,
                minor: None,
                access: "rw".to_string(),
            },
        ]
        .iter()
        .for_each(|d| {
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");
            let allowed_file = common::open_cgroup_file(tmp.join("devices.allow")).expect("open allowed devices");
            let denied_file = common::open_cgroup_file(tmp.join("devices.deny")).expect("open denied devices");

            aw!(Devices::apply_device(&ring, &d, &allowed_file, &denied_file)).expect("Apply default device");
            println!("Device: {}", d.to_string());
            if d.allow {
                let allowed_content =
                    read_to_string(tmp.join("devices.allow")).expect("read to string");
                assert_eq!(allowed_content, d.to_string());
            } else {
                let denied_content =
                    read_to_string(tmp.join("devices.deny")).expect("read to string");
                assert_eq!(denied_content, d.to_string());
            }
        });
    }

    quickcheck! {
        fn property_test_apply_device(device: LinuxDeviceCgroup) -> bool {
            let tmp = create_temp_dir("property_test_apply_device").expect("create temp directory for test");
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");
            let ring = rio::new().expect("start io_uring");
            let allowed_file = common::open_cgroup_file(tmp.join("devices.allow")).expect("open allowed devices");
            let denied_file = common::open_cgroup_file(tmp.join("devices.deny")).expect("open denied devices");

            aw!(Devices::apply_device(&ring, &device, &allowed_file, &denied_file)).expect("Apply default device");
            if device.allow {
                let allowed_content =
                    read_to_string(tmp.join("devices.allow")).expect("read to string");
                allowed_content == device.to_string()
            } else {
                let denied_content =
                    read_to_string(tmp.join("devices.deny")).expect("read to string");
                denied_content == device.to_string()
            }
        }

        fn property_test_apply_multiple_devices(devices: Vec<LinuxDeviceCgroup>) -> bool {
            let tmp = create_temp_dir("property_test_apply_multiple_devices").expect("create temp directory for test");
            devices.iter()
                .map(|device| {
                    set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
                    set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");
                    let ring = rio::new().expect("start io_uring");
                    let allowed_file = common::open_cgroup_file(tmp.join("devices.allow")).expect("open allowed devices");
                    let denied_file = common::open_cgroup_file(tmp.join("devices.deny")).expect("open denied devices");

                    aw!(Devices::apply_device(&ring, &device, &allowed_file, &denied_file)).expect("Apply default device");
                    if device.allow {
                        let allowed_content =
                            read_to_string(tmp.join("devices.allow")).expect("read to string");
                        allowed_content == device.to_string()
                    } else {
                        let denied_content =
                            read_to_string(tmp.join("devices.deny")).expect("read to string");
                        denied_content == device.to_string()
                    }
                })
                .all(|is_ok| is_ok)
        }
    }
}
