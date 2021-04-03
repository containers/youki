use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
pub struct LinuxIDMapping {
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
pub struct LinuxCPU {
    pub shares: Option<u64>,
    pub quota: Option<i64>,
    pub period: Option<u64>,
    pub realtime_runtime: Option<i64>,
    pub realtime_period: Option<u64>,
    #[serde(default)]
    pub cpus: String,
    #[serde(default)]
    pub mems: String,
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
pub struct LinuxBlockIO {
    pub blkio_weight: Option<u16>,
    pub blkio_leaf_weight: Option<u16>,
    #[serde(default)]
    pub blkio_weight_device: Vec<LinuxWeightDevice>,
    #[serde(default)]
    pub blkio_throttle_read_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(default)]
    pub blkio_throttle_write_bps_device: Vec<LinuxThrottleDevice>,
    #[serde(default, rename = "blkioThrottleReadIOPSDevice")]
    pub blkio_throttle_read_iops_device: Vec<LinuxThrottleDevice>,
    #[serde(default, rename = "blkioThrottleWriteIOPSDevice")]
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
    pub cpu: Option<LinuxCPU>,
    pub pids: Option<LinuxPids>,
    #[serde(rename = "blockIO")]
    pub block_io: Option<LinuxBlockIO>,
    #[serde(default)]
    pub hugepage_limits: Vec<LinuxHugepageLimit>,
    pub network: Option<LinuxNetwork>,
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
    #[serde(default)]
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDevice {
    #[serde(default)]
    pub path: String,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Linux {
    #[serde(default)]
    pub uid_mappings: Vec<LinuxIDMapping>,
    #[serde(default)]
    pub gid_mappings: Vec<LinuxIDMapping>,
    #[serde(default)]
    pub sysctl: HashMap<String, String>,
    pub resources: Option<LinuxResources>,
    #[serde(default)]
    pub cgroups_path: String,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub fn load(path: &str) -> Result<Self> {
        let file = File::open(path)?;
        let mut spec: Spec = serde_json::from_reader(&file)?;
        spec.root.path = std::fs::canonicalize(spec.root.path)?;
        Ok(spec)
    }
}
