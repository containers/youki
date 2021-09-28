use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;

use super::controller::Controller;
use crate::common::{self, default_allow_devices, default_devices, ControllerOpt};
use oci_spec::runtime::LinuxDeviceCgroup;

pub struct Devices {}

#[async_trait(?Send)]
impl Controller for Devices {
    type Resource = ();

    async fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Devices cgroup config");

        if let Some(devices) = controller_opt.resources.devices().as_ref() {
            for d in devices {
                Self::apply_device(d, cgroup_root).await?;
            }
        }

        for d in [
            default_devices().iter().map(|d| d.into()).collect(),
            default_allow_devices(),
        ]
        .concat()
        {
            Self::apply_device(&d, cgroup_root).await?;
        }

        Ok(())
    }

    // always needs to be called due to default devices
    fn needs_to_handle<'a>(_controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        Some(&())
    }
}

impl Devices {
    async fn apply_device(device: &LinuxDeviceCgroup, cgroup_root: &Path) -> Result<()> {
        let path = if device.allow() {
            cgroup_root.join("devices.allow")
        } else {
            cgroup_root.join("devices.deny")
        };

        common::async_write_cgroup_file_str(path, &device.to_string()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{aw, set_fixture, create_temp_dir};
    use oci_spec::runtime::{LinuxDeviceCgroupBuilder, LinuxDeviceType};
    use std::fs::read_to_string;

    #[test]
    fn test_set_default_devices() {
        let tmp =
            create_temp_dir("test_set_default_devices").expect("create temp directory for test");

        default_allow_devices().iter().for_each(|d| {
            // NOTE: We reset the fixtures every iteration because files aren't appended
            // so what happens in the tests is you get strange overwrites which can contain
            // remaining bytes from the last iteration. Resetting the files more appropriately
            // mocks the behavior of cgroup files.
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");

            aw!(Devices::apply_device(d, &tmp)).expect("Apply default device");
            println!("Device: {}", d.to_string());
            if d.allow() {
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
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(LinuxDeviceType::C)
                .major(10)
                .access("rwm")
                .build()
                .unwrap(),
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(LinuxDeviceType::A)
                .minor(200)
                .access("rwm")
                .build()
                .unwrap(),
            LinuxDeviceCgroupBuilder::default()
                .allow(false)
                .typ(LinuxDeviceType::P)
                .major(10)
                .minor(200)
                .access("m")
                .build()
                .unwrap(),
            LinuxDeviceCgroupBuilder::default()
                .allow(false)
                .typ(LinuxDeviceType::U)
                .access("rw")
                .build()
                .unwrap(),
        ]
        .iter()
        .for_each(|d| {
            set_fixture(&tmp, "devices.allow", "").expect("create allowed devices list");
            set_fixture(&tmp, "devices.deny", "").expect("create denied devices list");

            aw!(Devices::apply_device(d, &tmp)).expect("Apply default device");
            println!("Device: {}", d.to_string());
            if d.allow() {
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
            if device.allow() {
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
                    if device.allow() {
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
