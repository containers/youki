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

        // concat an array of default devices
        let defaults = [
            default_devices().iter().map(|d| d.into()).collect(),
            Self::default_allow_devices(),
        ].concat();

        // combine iterators for default and specified devices
        let devices_iter = linux_resources.devices.into_iter().chain(defaults.into_iter());

        // partition all devices into a allowed and denied device sets
        let (allowed, denied): (Vec<LinuxDeviceCgroup>, Vec<LinuxDeviceCgroup>) = devices_iter.into_iter().partition(|d| d.allow);
        
        // apply the allowed devices all at once
        Self::apply_allowed_devices(ring, &allowed, cgroup_root).await?;

        // apply the denied devices all at once
        Self::apply_denied_devices(ring, &denied, cgroup_root).await?;

        Ok(())
    }

    // always needs to be called due to default devices
    fn needs_to_handle(_linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        Some(&())
    }
}

impl Devices {
    async fn apply_allowed_devices(ring: &Rio, devices: &[LinuxDeviceCgroup], cgroup_root: &Path) -> Result<()> {
        let file = common::open_cgroup_file(cgroup_root.join("devices.allow"))?;
        Self::apply_devices(ring, devices, &file).await
    }

    async fn apply_denied_devices(ring: &Rio, devices: &[LinuxDeviceCgroup], cgroup_root: &Path) -> Result<()> {
        let file = common::open_cgroup_file(cgroup_root.join("device.deny"))?;
        Self::apply_devices(ring, devices, &file).await
    }

    async fn apply_devices(ring: &Rio, devices: &[LinuxDeviceCgroup], file: &File) -> Result<()> {
        // combine all device entries into a single string
        let contents = devices.iter().map(|d| d.to_string()).fold(String::new(), |a, b| a + &b + "\n");

        // apply all of the devices in one write
        common::async_write_cgroup_file_str(ring, file, &contents).await?;
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
    fn test_set_allowed_devices() {
        let tmp = create_temp_dir("test_set_allowed_devices").expect("create temp directory for test");
        let devices = [
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
                allow: true,
                typ: LinuxDeviceType::P,
                major: Some(10),
                minor: Some(200),
                access: "m".to_string(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: LinuxDeviceType::U,
                major: None,
                minor: None,
                access: "rw".to_string(),
            },
        ];
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");

            let ring = rio::new().expect("start io_uring");
            aw!(Devices::apply_allowed_devices(&ring, &devices, &tmp)).expect("Apply allowed device");
            let expected = devices.iter().map(|d| d.to_string()).fold(String::new(), |a, b| a + &b + "\n");
            let content = read_to_string(tmp.join("devices.allow")).expect("read file contents");
            assert_eq!(content, expected);
    }

    #[test]
    fn test_set_denied_devices() {
        let tmp = create_temp_dir("test_set_denied_devices").expect("create temp directory for test");
        let devices = [
            LinuxDeviceCgroup {
                allow: false,
                typ: LinuxDeviceType::C,
                major: Some(10),
                minor: None,
                access: "rwm".to_string(),
            },
            LinuxDeviceCgroup {
                allow: false,
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
        ];
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");

            let ring = rio::new().expect("start io_uring");
            aw!(Devices::apply_allowed_devices(&ring, &devices, &tmp)).expect("Apply denied device");
            let expected = devices.iter().map(|d| d.to_string()).fold(String::new(), |a, b| a + &b + "\n");
            let content = read_to_string(tmp.join("devices.denied")).expect("read file contents");
            assert_eq!(content, expected);
    }

    quickcheck! {
        fn property_test_apply_device(device: LinuxDeviceCgroup) -> bool {
            let tmp = create_temp_dir("property_test_apply_device").expect("create temp directory for test");
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");
            let ring = rio::new().expect("start io_uring");
            if device.allow {
                aw!(Devices::apply_allowed_devices(&ring, &[device], &tmp)).expect("Apply default device");
                let allowed_content =
                    read_to_string(tmp.join("devices.allow")).expect("read to string");
                allowed_content == device.to_string()
            } else {
                aw!(Devices::apply_denied_devices(&ring, &[device], &tmp)).expect("Apply default device");
                let denied_content =
                    read_to_string(tmp.join("devices.deny")).expect("read to string");
                denied_content == device.to_string()
            }
        }
    }
}
