use nix::sys::stat::SFlag;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Platform {
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub arch: String,
}

#[derive(Default, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Box {
    #[serde(default)]
    pub height: u64,
    #[serde(default)]
    pub width: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default)]
    pub additional_gids: Vec<u32>,
    #[serde(default)]
    pub username: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub console_size: Box,
    pub user: User,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub no_new_privileges: bool,
    #[serde(default)]
    pub apparmor_profile: String,
    #[serde(default)]
    pub selinux_label: String,
    #[serde(default, deserialize_with = "deserialize_caps")]
    pub capabilities: Option<LinuxCapabilities>,
    #[serde(default)]
    pub rlimits: Vec<LinuxRlimit>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxRlimit {
    #[serde(rename = "type")]
    pub typ: LinuxRlimitType,
    #[serde(default)]
    pub hard: u64,
    #[serde(default)]
    pub soft: u64,
}
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LinuxRlimitType {
    RlimitCpu,
    RlimitFsize,
    RlimitData,
    RlimitStack,
    RlimitCore,
    RlimitRss,
    RlimitNproc,
    RlimitNofile,
    RlimitMemlock,
    RlimitAs,
    RlimitLocks,
    RlimitSigpending,
    RlimitMsgqueue,
    RlimitNice,
    RlimitRtprio,
    RlimitRttime,
}

use caps::Capability;
#[derive(Debug, Clone)]
pub struct LinuxCapabilityType {
    pub cap: Capability,
}

impl<'de> Deserialize<'de> for LinuxCapabilityType {
    fn deserialize<D>(desirializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let r: serde_json::Value = serde::Deserialize::deserialize(desirializer)?;
        match r {
            serde_json::Value::String(type_string) => {
                let cap = match type_string.as_str() {
                    "CAP_CHOWN" => Capability::CAP_CHOWN,
                    "CAP_DAC_OVERRIDE" => Capability::CAP_DAC_OVERRIDE,
                    "CAP_DAC_READ_SEARCH" => Capability::CAP_DAC_READ_SEARCH,
                    "CAP_FOWNER" => Capability::CAP_FOWNER,
                    "CAP_FSETID" => Capability::CAP_FSETID,
                    "CAP_KILL" => Capability::CAP_KILL,
                    "CAP_SETGID" => Capability::CAP_SETGID,
                    "CAP_SETUID" => Capability::CAP_SETUID,
                    "CAP_SETPCAP" => Capability::CAP_SETPCAP,
                    "CAP_LINUX_IMMUTABLE" => Capability::CAP_LINUX_IMMUTABLE,
                    "CAP_NET_BIND_SERVICE" => Capability::CAP_NET_BIND_SERVICE,
                    "CAP_NET_BROADCAST" => Capability::CAP_NET_BROADCAST,
                    "CAP_NET_ADMIN" => Capability::CAP_NET_ADMIN,
                    "CAP_NET_RAW" => Capability::CAP_NET_RAW,
                    "CAP_IPC_LOCK" => Capability::CAP_IPC_LOCK,
                    "CAP_IPC_OWNER" => Capability::CAP_IPC_OWNER,
                    "CAP_SYS_MODULE" => Capability::CAP_SYS_MODULE,
                    "CAP_SYS_RAWIO" => Capability::CAP_SYS_RAWIO,
                    "CAP_SYS_CHROOT" => Capability::CAP_SYS_CHROOT,
                    "CAP_SYS_PTRACE" => Capability::CAP_SYS_PTRACE,
                    "CAP_SYS_PACCT" => Capability::CAP_SYS_PACCT,
                    "CAP_SYS_ADMIN" => Capability::CAP_SYS_ADMIN,
                    "CAP_SYS_BOOT" => Capability::CAP_SYS_BOOT,
                    "CAP_SYS_NICE" => Capability::CAP_SYS_NICE,
                    "CAP_SYS_RESOURCE" => Capability::CAP_SYS_RESOURCE,
                    "CAP_SYS_TIME" => Capability::CAP_SYS_TIME,
                    "CAP_SYS_TTYCONFIG" => Capability::CAP_SYS_TTY_CONFIG,
                    "CAP_SYSLOG" => Capability::CAP_SYSLOG,
                    "CAP_MKNOD" => Capability::CAP_MKNOD,
                    "CAP_LEASE" => Capability::CAP_LEASE,
                    "CAP_AUDIT_WRITE" => Capability::CAP_AUDIT_WRITE,
                    "CAP_AUDIT_CONTROL" => Capability::CAP_AUDIT_CONTROL,
                    "CAP_AUDIT_READ" => Capability::CAP_AUDIT_READ,
                    "CAP_SETFCAP" => Capability::CAP_SETFCAP,
                    "CAP_MAC_OVERRIDE" => Capability::CAP_MAC_OVERRIDE,
                    "CAP_MAC_ADMIN" => Capability::CAP_MAC_ADMIN,
                    "CAP_WAKE_ALARM" => Capability::CAP_WAKE_ALARM,
                    "CAP_BLOCK_SUSPEND" => Capability::CAP_BLOCK_SUSPEND,
                    unknown_cap => {
                        return Err(serde::de::Error::custom(format!(
                            "{:?} is unexpected type in capabilites",
                            unknown_cap
                        )))
                    }
                };
                Ok(LinuxCapabilityType { cap })
            }
            _ => Err(serde::de::Error::custom("Unexpected type in capabilites")),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct LinuxCapabilities {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bounding: Vec<LinuxCapabilityType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effective: Vec<LinuxCapabilityType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inheritable: Vec<LinuxCapabilityType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub permitted: Vec<LinuxCapabilityType>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ambient: Vec<LinuxCapabilityType>,
}

fn deserialize_caps<'de, D>(desirializer: D) -> Result<Option<LinuxCapabilities>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let r: serde_json::Value = serde::Deserialize::deserialize(desirializer)?;
    match r {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Array(a) => {
            let caps = cap_from_array::<D>(&a)?;
            let capabilities = LinuxCapabilities {
                bounding: caps.clone(),
                effective: caps.clone(),
                inheritable: caps.clone(),
                permitted: caps.clone(),
                ambient: caps,
            };

            Ok(Some(capabilities))
        }
        serde_json::Value::Object(o) => {
            let capabilities = LinuxCapabilities {
                bounding: cap_from_object::<D>(&o, "bounding")?,
                effective: cap_from_object::<D>(&o, "effective")?,
                inheritable: cap_from_object::<D>(&o, "inheritable")?,
                permitted: cap_from_object::<D>(&o, "permitted")?,
                ambient: cap_from_object::<D>(&o, "ambient")?,
            };

            Ok(Some(capabilities))
        }
        _ => Err(serde::de::Error::custom("Unexpected value in capabilites")),
    }
}

fn cap_from_object<'de, D>(
    o: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Vec<LinuxCapabilityType>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    if let Some(v) = o.get(key) {
        match *v {
            serde_json::Value::Null => Ok(Vec::new()),
            serde_json::Value::Array(ref a) => cap_from_array::<D>(a),
            _ => Err(serde::de::Error::custom(
                "Unexpected value in capability set",
            )),
        }
    } else {
        Ok(Vec::new())
    }
}

fn cap_from_array<'de, D>(a: &[serde_json::Value]) -> Result<Vec<LinuxCapabilityType>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let mut caps = Vec::new();
    for c in a {
        match LinuxCapabilityType::deserialize(c) {
            Ok(val) => caps.push(val),
            Err(_) => {
                let msg = format!("Capability '{}' is not valid", c);
                return Err(serde::de::Error::custom(msg));
            }
        }
    }
    Ok(caps)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Root {
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mount {
    #[serde(default)]
    pub destination: PathBuf,
    #[serde(default, rename = "type")]
    pub typ: String,
    #[serde(default)]
    pub source: PathBuf,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxIdMapping {
    #[serde(default, rename = "hostID")]
    pub host_id: u32,
    #[serde(default, rename = "containerID")]
    pub container_id: u32,
    #[serde(default)]
    pub size: u32,
}

// a is for LinuxDeviceCgroup
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum LinuxDeviceType {
    B,
    C,
    U,
    P,
    A,
}

impl Default for LinuxDeviceType {
    fn default() -> LinuxDeviceType {
        LinuxDeviceType::A
    }
}

impl LinuxDeviceType {
    pub fn to_sflag(&self) -> Result<SFlag> {
        Ok(match self {
            Self::B => SFlag::S_IFBLK,
            Self::C | LinuxDeviceType::U => SFlag::S_IFCHR,
            Self::P => SFlag::S_IFIFO,
            Self::A => bail!("type a is not allowed for linux device"),
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::B => "b",
            Self::C => "c",
            Self::U => "u",
            Self::P => "p",
            Self::A => "a",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxDeviceCgroup {
    #[serde(default)]
    pub allow: bool,
    #[serde(default, rename = "type")]
    pub typ: LinuxDeviceType,
    pub major: Option<i64>,
    pub minor: Option<i64>,
    #[serde(default)]
    pub access: String,
}

impl ToString for LinuxDeviceCgroup {
    fn to_string(&self) -> String {
        let major = self
            .major
            .map(|mj| mj.to_string())
            .unwrap_or_else(|| "*".to_string());
        let minor = self
            .minor
            .map(|mi| mi.to_string())
            .unwrap_or_else(|| "*".to_string());
        format!(
            "{} {}:{} {}",
            self.typ.as_str(),
            &major,
            &minor,
            &self.access
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxMemory {
    pub limit: Option<i64>,
    pub reservation: Option<i64>,
    pub swap: Option<i64>,
    pub kernel: Option<i64>,
    #[serde(rename = "kernelTCP")]
    pub kernel_tcp: Option<i64>,
    pub swappiness: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxCpu {
    pub shares: Option<u64>,
    pub quota: Option<i64>,
    pub period: Option<u64>,
    pub realtime_runtime: Option<i64>,
    pub realtime_period: Option<u64>,
    #[serde(default)]
    pub cpus: Option<String>,
    #[serde(default)]
    pub mems: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxPids {
    #[serde(default)]
    pub limit: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxWeightDevice {
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    pub weight: Option<u16>,
    pub leaf_weight: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxThrottleDevice {
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    #[serde(default)]
    pub rate: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxBlockIo {
    pub blkio_weight: Option<u16>,
    pub blkio_leaf_weight: Option<u16>,
    #[serde(default)]
    pub blkio_weight_device: Vec<LinuxWeightDevice>,
    #[serde(default, rename = "throttleReadBpsDevice")]
    pub blkio_throttle_read_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(default, rename = "throttleWriteBpsDevice")]
    pub blkio_throttle_write_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(default, rename = "throttleReadIOPSDevice")]
    pub blkio_throttle_read_iops_device: Vec<LinuxThrottleDevice>,
    #[serde(default, rename = "throttleWriteIOPSDevice")]
    pub blkio_throttle_write_iops_device: Vec<LinuxThrottleDevice>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxHugepageLimit {
    #[serde(default)]
    pub page_size: String,
    #[serde(default)]
    pub limit: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxInterfacePriority {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub priority: u32,
}

impl ToString for LinuxInterfacePriority {
    fn to_string(&self) -> String {
        format!("{} {}\n", self.name, self.priority)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxNetwork {
    #[serde(rename = "classID")]
    pub class_id: Option<u32>,
    #[serde(default)]
    pub priorities: Vec<LinuxInterfacePriority>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxResources {
    #[serde(default)]
    pub devices: Vec<LinuxDeviceCgroup>,
    #[serde(default)]
    pub disable_oom_killer: bool,
    pub oom_score_adj: Option<i32>,
    pub memory: Option<LinuxMemory>,
    pub cpu: Option<LinuxCpu>,
    pub pids: Option<LinuxPids>,
    #[serde(rename = "blockIO")]
    pub block_io: Option<LinuxBlockIo>,
    #[serde(default)]
    pub hugepage_limits: Vec<LinuxHugepageLimit>,
    pub network: Option<LinuxNetwork>,
    pub freezer: Option<FreezerState>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LinuxNamespaceType {
    Mount = 0x00020000,
    Cgroup = 0x02000000,
    Uts = 0x04000000,
    Ipc = 0x08000000,
    User = 0x10000000,
    Pid = 0x20000000,
    Network = 0x40000000,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinuxNamespace {
    #[serde(rename = "type")]
    pub typ: LinuxNamespaceType,
    pub path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDevice {
    #[serde(default)]
    pub path: PathBuf,
    #[serde(rename = "type")]
    pub typ: LinuxDeviceType,
    #[serde(default)]
    pub major: u64,
    #[serde(default)]
    pub minor: u64,
    pub file_mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl From<&LinuxDevice> for LinuxDeviceCgroup {
    fn from(linux_device: &LinuxDevice) -> LinuxDeviceCgroup {
        LinuxDeviceCgroup {
            allow: true,
            typ: linux_device.typ,
            major: Some(linux_device.major as i64),
            minor: Some(linux_device.minor as i64),
            access: "rwm".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u32)]
pub enum LinuxSeccompAction {
    ScmpActKill = 0x00000000,
    ScmpActTrap = 0x00030000,
    ScmpActErrno = 0x00050001,
    ScmpActTrace = 0x7ff00001,
    ScmpActAllow = 0x7fff0000,
}

#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Arch {
    ScmpArchNative = 0x00000000,
    ScmpArchX86 = 0x40000003,
    ScmpArchX86_64 = 0xc000003e,
    ScmpArchX32 = 0x4000003e,
    ScmpArchArm = 0x40000028,
    ScmpArchAarch64 = 0xc00000b7,
    ScmpArchMips = 0x00000008,
    ScmpArchMips64 = 0x80000008,
    ScmpArchMips64n32 = 0xa0000008,
    ScmpArchMipsel = 0x40000008,
    ScmpArchMipsel64 = 0xc0000008,
    ScmpArchMipsel64n32 = 0xe0000008,
    ScmpArchPpc = 0x00000014,
    ScmpArchPpc64 = 0x80000015,
    ScmpArchPpc64le = 0xc0000015,
    ScmpArchS390 = 0x00000016,
    ScmpArchS390x = 0x80000016,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(u32)]
pub enum LinuxSeccompOperator {
    ScmpCmpNe = 1,
    ScmpCmpLt = 2,
    ScmpCmpLe = 3,
    ScmpCmpEq = 4,
    ScmpCmpGe = 5,
    ScmpCmpGt = 6,
    ScmpCmpMaskedEq = 7,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezerState {
    Undefined,
    Frozen,
    Thawed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Linux {
    #[serde(default)]
    pub uid_mappings: Vec<LinuxIdMapping>,
    #[serde(default)]
    pub gid_mappings: Vec<LinuxIdMapping>,
    #[serde(default)]
    pub sysctl: HashMap<String, String>,
    pub resources: Option<LinuxResources>,
    #[serde(default)]
    pub cgroups_path: Option<PathBuf>,
    #[serde(default)]
    pub namespaces: Vec<LinuxNamespace>,
    #[serde(default)]
    pub devices: Vec<LinuxDevice>,
    #[serde(default)]
    pub rootfs_propagation: String,
    #[serde(default)]
    pub masked_paths: Vec<String>,
    #[serde(default)]
    pub readonly_paths: Vec<String>,
    #[serde(default)]
    pub mount_label: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Spec {
    #[serde(default, rename = "ociVersion")]
    pub version: String,
    pub platform: Option<Platform>,
    pub process: Process,
    pub root: Root,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub mounts: Vec<Mount>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    pub linux: Option<Linux>,
}

impl Spec {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("load spec: failed to open {:?}", path))?;
        let spec: Spec = serde_json::from_reader(&file)?;
        Ok(spec)
    }

    pub fn canonicalize_rootfs(&mut self) -> Result<()> {
        self.root.path = std::fs::canonicalize(&self.root.path)
            .with_context(|| format!("failed to canonicalize {:?}", self.root.path))?;
        Ok(())
    }
}

#[cfg(feature = "proptests")]
use quickcheck::{Arbitrary, Gen};

#[cfg(feature = "proptests")]
fn some_none_generator_util<T: Arbitrary>(g: &mut Gen) -> Option<T> {
    let choice = g.choose(&[true, false]).unwrap();
    match choice {
        false => None,
        true => Some(T::arbitrary(g)),
    }
}

#[cfg(feature = "proptests")]
impl Arbitrary for LinuxDeviceCgroup {
    fn arbitrary(g: &mut Gen) -> LinuxDeviceCgroup {
        let typ_choices = ["b", "c", "u", "p", "a"];

        let typ_chosen = g.choose(&typ_choices).unwrap();

        let typ = match typ_chosen.to_string().as_str() {
            "b" => LinuxDeviceType::B,
            "c" => LinuxDeviceType::C,
            "u" => LinuxDeviceType::U,
            "p" => LinuxDeviceType::P,
            "a" => LinuxDeviceType::A,
            _ => LinuxDeviceType::A,
        };

        let access_choices = ["rwm", "m"];
        LinuxDeviceCgroup {
            allow: bool::arbitrary(g),
            typ,
            major: some_none_generator_util::<i64>(g),
            minor: some_none_generator_util::<i64>(g),
            access: g.choose(&access_choices).unwrap().to_string(),
        }
    }
}

#[cfg(feature = "proptests")]
impl Arbitrary for LinuxMemory {
    fn arbitrary(g: &mut Gen) -> LinuxMemory {
        LinuxMemory {
            kernel: some_none_generator_util::<i64>(g),
            kernel_tcp: some_none_generator_util::<i64>(g),
            limit: some_none_generator_util::<i64>(g),
            reservation: some_none_generator_util::<i64>(g),
            swap: some_none_generator_util::<i64>(g),
            swappiness: some_none_generator_util::<u64>(g),
        }
    }
}

#[cfg(feature = "proptests")]
impl Arbitrary for LinuxHugepageLimit {
    fn arbitrary(g: &mut Gen) -> LinuxHugepageLimit {
        let unit_choice = ["KB", "MB", "GB"];
        let unit = g.choose(&unit_choice).unwrap();
        let page_size = u64::arbitrary(g).to_string() + unit;

        LinuxHugepageLimit {
            page_size,
            limit: i64::arbitrary(g),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
