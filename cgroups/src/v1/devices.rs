use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;

use super::controller::Controller;
use crate::common;
use oci_spec::{LinuxDevice, LinuxDeviceCgroup, LinuxDeviceType, LinuxResources};

pub struct Devices {}

#[async_trait(?Send)]
impl Controller for Devices {
    type Resource = ();

    async fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Devices cgroup config");

        if let Some(devices) = linux_resources.devices.as_ref() {
            for d in devices {
                Self::apply_device(d, cgroup_root).await?;
            }
        }

        for d in [
            Self::default_devices().iter().map(|d| d.into()).collect(),
            Self::default_allow_devices(),
        ]
        .concat()
        {
            Self::apply_device(&d, cgroup_root).await?;
        }

        Ok(())
    }

    // always needs to be called due to default devices
    fn needs_to_handle(_linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        Some(&())
    }
}

impl Devices {
    async fn apply_device(device: &LinuxDeviceCgroup, cgroup_root: &Path) -> Result<()> {
        let path = if device.allow {
            cgroup_root.join("devices.allow")
        } else {
            cgroup_root.join("devices.deny")
        };

        common::async_write_cgroup_file_str(path, &device.to_string()).await?;
        Ok(())
    }

    fn default_allow_devices() -> Vec<LinuxDeviceCgroup> {
        vec![
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: None,
                minor: None,
                access: "m".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::B),
                major: None,
                minor: None,
                access: "m".to_string().into(),
            },
            // /dev/console
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(5),
                minor: Some(1),
                access: "rwm".to_string().into(),
            },
            // /dev/pts
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(136),
                minor: None,
                access: "rwm".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(5),
                minor: Some(2),
                access: "rwm".to_string().into(),
            },
            // tun/tap
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(10),
                minor: Some(200),
                access: "rwm".to_string().into(),
            },
        ]
    }

    pub fn default_devices() -> Vec<LinuxDevice> {
        vec![
            LinuxDevice {
                path: PathBuf::from("/dev/null"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 3,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/zero"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 5,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/full"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 7,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/tty"),
                typ: LinuxDeviceType::C,
                major: 5,
                minor: 0,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/urandom"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 9,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/random"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 8,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::create_temp_dir;
    use crate::test::{aw, set_fixture};
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

            aw!(Devices::apply_device(d, &tmp)).expect("Apply default device");
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
        [
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(10),
                minor: None,
                access: "rwm".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::A),
                major: None,
                minor: Some(200),
                access: "rwm".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: false,
                typ: Some(LinuxDeviceType::P),
                major: Some(10),
                minor: Some(200),
                access: "m".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: false,
                typ: Some(LinuxDeviceType::U),
                major: None,
                minor: None,
                access: "rw".to_string().into(),
            },
        ]
        .iter()
        .for_each(|d| {
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");

            aw!(Devices::apply_device(d, &tmp)).expect("Apply default device");
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
            aw!(Devices::apply_device(&device, &tmp)).expect("Apply default device");
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
                    aw!(Devices::apply_device(device, &tmp)).expect("Apply default device");
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
