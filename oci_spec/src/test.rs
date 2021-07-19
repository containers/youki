#[cfg(test)]
use super::*;

#[test]
fn test_caps_to_linux_caps() {
    let spec: Spec = Default::default();
    if let Some(linux) = spec.process.capabilities {
        let linux_caps = linux.bounding[0];
        let convert_caps: Capability = linux_caps.into();
        assert_eq!(convert_caps, Capability::CAP_AUDIT_WRITE);
        assert_eq!(
            linux_caps,
            LinuxCapabilityType::from(Capability::CAP_AUDIT_WRITE)
        );
    }
}

#[test]
fn serialize_and_deserialize_spec() {
    let spec: Spec = Default::default();
    let json_string = serde_json::to_string(&spec).unwrap();
    let new_spec = serde_json::from_str(&json_string).unwrap();
    assert_eq!(spec, new_spec);
}

#[test]
fn test_linux_device_cgroup_to_string() {
    let ldc = LinuxDeviceCgroup {
        allow: true,
        typ: LinuxDeviceType::A,
        major: None,
        minor: None,
        access: "rwm".into(),
    };
    assert_eq!(ldc.to_string(), "a *:* rwm");
    let ldc = LinuxDeviceCgroup {
        allow: true,
        typ: LinuxDeviceType::A,
        major: Some(1),
        minor: Some(9),
        access: "rwm".into(),
    };
    assert_eq!(ldc.to_string(), "a 1:9 rwm");
}
