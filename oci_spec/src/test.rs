#[cfg(test)]
use super::*;

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
        typ: Some(LinuxDeviceType::A),
        major: None,
        minor: None,
        access: Some("rwm".into()),
    };
    assert_eq!(ldc.to_string(), "a *:* rwm");
    let ldc = LinuxDeviceCgroup {
        allow: true,
        typ: Some(LinuxDeviceType::A),
        major: Some(1),
        minor: Some(9),
        access: Some("rwm".into()),
    };
    assert_eq!(ldc.to_string(), "a 1:9 rwm");
}
