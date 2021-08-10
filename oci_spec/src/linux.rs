use anyhow::{bail, Result};
use nix::sys::stat::SFlag;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::TryFrom, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Linux contains platform-specific configuration for Linux based containers.
pub struct Linux {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// UIDMappings specifies user mappings for supporting user namespaces.
    pub uid_mappings: Option<Vec<LinuxIdMapping>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// GIDMappings specifies group mappings for supporting user namespaces.
    pub gid_mappings: Option<Vec<LinuxIdMapping>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Sysctl are a set of key value pairs that are set for the container on start.
    pub sysctl: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resources contain cgroup information for handling resource constraints for the container.
    pub resources: Option<LinuxResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CgroupsPath specifies the path to cgroups that are created and/or joined by the container.
    /// The path is expected to be relative to the cgroups mountpoint. If resources are specified,
    /// the cgroups at CgroupsPath will be updated based on resources.
    pub cgroups_path: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Namespaces contains the namespaces that are created and/or joined by the container.
    pub namespaces: Option<Vec<LinuxNamespace>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Devices are a list of device nodes that are created for the container.
    pub devices: Option<Vec<LinuxDevice>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seccomp specifies the seccomp security settings for the container.
    pub seccomp: Option<LinuxSeccomp>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// RootfsPropagation is the rootfs mount propagation mode for the container.
    pub rootfs_propagation: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// MaskedPaths masks over the provided paths inside the container.
    pub masked_paths: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// ReadonlyPaths sets the provided paths as RO inside the container.
    pub readonly_paths: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// MountLabel specifies the selinux context for the mounts in the container.
    pub mount_label: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// IntelRdt contains Intel Resource Director Technology (RDT) information for handling
    /// resource constraints (e.g., L3 cache, memory bandwidth) for the container.
    pub intel_rdt: Option<LinuxIntelRdt>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Personality contains configuration for the Linux personality syscall.
    pub personality: Option<LinuxPersonality>,
}

// Default impl for Linux (see funtions for more info)
impl Default for Linux {
    fn default() -> Self {
        Linux {
            // Creates empty Vec
            uid_mappings: Default::default(),
            // Creates empty Vec
            gid_mappings: Default::default(),
            // Empty sysctl Hashmap
            sysctl: Default::default(),
            resources: Some(LinuxResources {
                devices: vec![LinuxDeviceCgroup {
                    access: "rwm".to_string().into(),
                    allow: false,
                    typ: Default::default(),
                    major: Default::default(),
                    minor: Default::default(),
                }]
                .into(),
                disable_oom_killer: Default::default(),
                oom_score_adj: Default::default(),
                memory: Default::default(),
                cpu: Default::default(),
                pids: Default::default(),
                block_io: Default::default(),
                hugepage_limits: Default::default(),
                network: Default::default(),
                freezer: Default::default(),
                rdma: Default::default(),
                unified: Default::default(),
            }),
            // Defaults to None
            cgroups_path: Default::default(),
            namespaces: get_default_namespaces().into(),
            // Empty Vec
            devices: Default::default(),
            // Empty String
            rootfs_propagation: Default::default(),
            masked_paths: get_default_maskedpaths().into(),
            readonly_paths: get_default_readonly_paths().into(),
            // Empty String
            mount_label: Default::default(),
            seccomp: None,
            intel_rdt: None,
            personality: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxIDMapping specifies UID/GID mappings.
pub struct LinuxIdMapping {
    #[serde(default, rename = "hostID")]
    /// HostID is the starting UID/GID on the host to be mapped to `container_id`.
    pub host_id: u32,

    #[serde(default, rename = "containerID")]
    /// ContainerID is the starting UID/GID in the container.
    pub container_id: u32,

    #[serde(default)]
    /// Size is the number of IDs to be mapped.
    pub size: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Device types
pub enum LinuxDeviceType {
    /// block (buffered)
    B,

    /// character (unbuffered)
    C,

    /// character (unbufferd)
    U,

    /// FIFO
    P,

    /// ??
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

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// Represents a device rule for the devices specified to the device controller
pub struct LinuxDeviceCgroup {
    #[serde(default)]
    /// Allow or deny
    pub allow: bool,

    /// Device type, block, char, etc.
    #[serde(default, rename = "type")]
    pub typ: Option<LinuxDeviceType>,

    /// Device's major number
    pub major: Option<i64>,

    /// Device's minor number
    pub minor: Option<i64>,

    /// Cgroup access premissions format, rwm.
    #[serde(default)]
    pub access: Option<String>,
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
        let access = self.access.as_deref().unwrap_or("");
        format!(
            "{} {}:{} {}",
            &self.typ.unwrap_or_default().as_str(),
            &major,
            &minor,
            &access
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxMemory for Linux cgroup 'memory' resource management.
pub struct LinuxMemory {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Memory limit (in bytes).
    pub limit: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Memory reservation or soft_limit (in bytes).
    pub reservation: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Total memory limit (memory + swap).
    pub swap: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Kernel memory limit (in bytes).
    pub kernel: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "kernelTCP")]
    /// Kernel memory limit for tcp (in bytes).
    pub kernel_tcp: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// How aggressive the kernel will swap memory pages.
    pub swappiness: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "disableOOMKiller")]
    /// DisableOOMKiller disables the OOM killer for out of memory conditions.
    pub disable_oom_killer: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Enables hierarchical memory accounting
    pub use_hierarchy: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxCPU for Linux cgroup 'cpu' resource management.
pub struct LinuxCpu {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// CPU shares (relative weight (ratio) vs. other cgroups with cpu shares).
    pub shares: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// CPU hardcap limit (in usecs). Allowed cpu time in a given period.
    pub quota: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// CPU period to be used for hardcapping (in usecs).
    pub period: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// How much time realtime scheduling may use (in usecs).
    pub realtime_runtime: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// CPU period to be used for realtime scheduling (in usecs).
    pub realtime_period: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CPUs to use within the cpuset. Default is to use any CPU available.
    pub cpus: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// List of memory nodes in the cpuset. Default is to use any available memory node.
    pub mems: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxPids for Linux cgroup 'pids' resource management (Linux 4.3).
pub struct LinuxPids {
    #[serde(default)]
    /// Maximum number of PIDs. Default is "no limit".
    pub limit: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxWeightDevice struct holds a `major:minor weight` pair for weightDevice.
pub struct LinuxWeightDevice {
    #[serde(default)]
    /// Major is the device's major number.
    pub major: i64,

    #[serde(default)]
    /// Minor is the device's minor number.
    pub minor: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Weight is the bandwidth rate for the device.
    pub weight: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// LeafWeight is the bandwidth rate for the device while competing with the cgroup's child
    /// cgroups, CFQ scheduler only.
    pub leaf_weight: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxThrottleDevice struct holds a `major:minor rate_per_second` pair.
pub struct LinuxThrottleDevice {
    #[serde(default)]
    /// Major is the device's major number.
    pub major: i64,

    #[serde(default)]
    /// Minor is the device's minor number.
    pub minor: i64,

    #[serde(default)]
    /// Rate is the IO rate limit per cgroup per device.
    pub rate: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxBlockIO for Linux cgroup 'blkio' resource management.
pub struct LinuxBlockIo {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Specifies per cgroup weight.
    pub weight: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Specifies tasks' weight in the given cgroup while competing with the cgroup's child
    /// cgroups, CFQ scheduler only.
    pub leaf_weight: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Weight per cgroup per device, can override BlkioWeight.
    pub weight_device: Option<Vec<LinuxWeightDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// IO read rate limit per cgroup per device, bytes per second.
    pub throttle_read_bps_device: Option<Vec<LinuxThrottleDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// IO write rate limit per cgroup per device, bytes per second.
    pub throttle_write_bps_device: Option<Vec<LinuxThrottleDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// IO read rate limit per cgroup per device, IO per second.
    pub throttle_read_iops_device: Option<Vec<LinuxThrottleDevice>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// IO write rate limit per cgroup per device, IO per second.
    pub throttle_write_iops_device: Option<Vec<LinuxThrottleDevice>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxHugepageLimit structure corresponds to limiting kernel hugepages.
pub struct LinuxHugepageLimit {
    #[serde(default)]
    /// Pagesize is the hugepage size.
    /// Format: "<size><unit-prefix>B' (e.g. 64KB, 2MB, 1GB, etc.)
    pub page_size: String,

    #[serde(default)]
    /// Limit is the limit of "hugepagesize" hugetlb usage.
    pub limit: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxInterfacePriority for network interfaces.
pub struct LinuxInterfacePriority {
    #[serde(default)]
    /// Name is the name of the network interface.
    pub name: String,

    #[serde(default)]
    /// Priority for the interface.
    pub priority: u32,
}

impl ToString for LinuxInterfacePriority {
    fn to_string(&self) -> String {
        format!("{} {}\n", self.name, self.priority)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxNetwork identification and priority configuration.
pub struct LinuxNetwork {
    #[serde(skip_serializing_if = "Option::is_none", rename = "classID")]
    /// Set class identifier for container's network packets
    pub class_id: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Set priority of network traffic for container.
    pub priorities: Option<Vec<LinuxInterfacePriority>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Resource constraints for container
pub struct LinuxResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Devices configures the device allowlist.
    pub devices: Option<Vec<LinuxDeviceCgroup>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Memory restriction configuration.
    pub memory: Option<LinuxMemory>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CPU resource restriction configuration.
    pub cpu: Option<LinuxCpu>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Task resource restrictions
    pub pids: Option<LinuxPids>,

    #[serde(default, skip_serializing_if = "Option::is_none", rename = "blockIO")]
    /// BlockIO restriction configuration.
    pub block_io: Option<LinuxBlockIo>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Hugetlb limit (in bytes).
    pub hugepage_limits: Option<Vec<LinuxHugepageLimit>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Network restriction configuration.
    pub network: Option<LinuxNetwork>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Rdma resource restriction configuration. Limits are a set of key value pairs that define
    /// RDMA resource limits, where the key is device name and value is resource limits.
    pub rdma: Option<HashMap<String, LinuxRdma>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Unified resources.
    pub unified: Option<HashMap<String, String>>,

    // TODO: I am not part of the official spec
    #[serde(default)]
    // Disables the OOM killer for out of memory conditions
    pub disable_oom_killer: bool,

    // TODO: I am not part of the official spec
    // Specify an oom_score_adj for container
    pub oom_score_adj: Option<i32>,

    // TODO: I am not part of the official spec
    pub freezer: Option<FreezerState>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxRdma for Linux cgroup 'rdma' resource management (Linux 4.11).
pub struct LinuxRdma {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum number of HCA handles that can be opened. Default is "no limit".
    hca_handles: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Maximum number of HCA objects that can be created. Default is "no limit".
    hca_objects: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxNamespaceType {
    /// Mount Namespace for isolating mount points
    Mount = 0x00020000,

    /// Cgroup Namespace for isolating cgroup hierarchies
    Cgroup = 0x02000000,

    /// Uts Namespace for isolating hostname and NIS domain name
    Uts = 0x04000000,

    /// Ipc Namespace for isolating System V, IPC, POSIX message queues
    Ipc = 0x08000000,

    /// User Namespace for isolating user and group  ids
    User = 0x10000000,

    /// PID Namespace for isolating process ids
    Pid = 0x20000000,

    /// Network Namespace for isolating network devices, ports, stacks etc.
    Network = 0x40000000,
}

impl TryFrom<&str> for LinuxNamespaceType {
    type Error = anyhow::Error;

    fn try_from(namespace: &str) -> Result<Self, Self::Error> {
        match namespace {
            "mnt" => Ok(LinuxNamespaceType::Mount),
            "cgroup" => Ok(LinuxNamespaceType::Cgroup),
            "uts" => Ok(LinuxNamespaceType::Uts),
            "ipc" => Ok(LinuxNamespaceType::Ipc),
            "user" => Ok(LinuxNamespaceType::User),
            "pid" => Ok(LinuxNamespaceType::Pid),
            "net" => Ok(LinuxNamespaceType::Network),
            _ => bail!("unknown namespace {}, could not convert", namespace),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxNamespace is the configuration for a Linux namespace.
pub struct LinuxNamespace {
    #[serde(rename = "type")]
    /// Type is the type of namespace.
    pub typ: LinuxNamespaceType,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Path is a path to an existing namespace persisted on disk that can be joined and is of the
    /// same type
    pub path: Option<String>,
}

// Utility function to get default namespaces
pub fn get_default_namespaces() -> Vec<LinuxNamespace> {
    vec![
        LinuxNamespace {
            typ: LinuxNamespaceType::Pid,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Network,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Ipc,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Uts,
            path: Default::default(),
        },
        LinuxNamespace {
            typ: LinuxNamespaceType::Mount,
            path: Default::default(),
        },
    ]
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxDevice represents the mknod information for a Linux special device file.
pub struct LinuxDevice {
    #[serde(default)]
    /// Path to the device.
    pub path: PathBuf,

    #[serde(rename = "type")]
    /// Device type, block, char, etc..
    pub typ: LinuxDeviceType,

    #[serde(default)]
    /// Major is the device's major number.
    pub major: i64,

    #[serde(default)]
    /// Minor is the device's minor number.
    pub minor: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// FileMode permission bits for the device.
    pub file_mode: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// UID of the device.
    pub uid: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Gid of the device.
    pub gid: Option<u32>,
}

impl From<&LinuxDevice> for LinuxDeviceCgroup {
    fn from(linux_device: &LinuxDevice) -> LinuxDeviceCgroup {
        LinuxDeviceCgroup {
            allow: true,
            typ: linux_device.typ.into(),
            major: Some(linux_device.major as i64),
            minor: Some(linux_device.minor as i64),
            access: "rwm".to_string().into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxSeccomp represents syscall restrictions.
pub struct LinuxSeccomp {
    pub default_action: LinuxSeccompAction,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architectures: Option<Vec<Arch>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syscalls: Option<Vec<LinuxSyscall>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxSyscall is used to match a syscall in seccomp.
pub struct LinuxSyscall {
    pub names: Vec<String>,

    pub action: LinuxSeccompAction,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errno_ret: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<LinuxSeccompArg>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxSeccompArg used for matching specific syscall arguments in seccomp.
pub struct LinuxSeccompArg {
    pub index: usize,

    pub value: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_two: Option<u64>,

    pub op: LinuxSeccompOperator,
}

// Default masks paths, cannot read these host files
pub fn get_default_maskedpaths() -> Vec<String> {
    vec![
        // For example now host interfaces such as
        // bluetooth cannot be accessed due to /proc/acpi
        "/proc/acpi".to_string(),
        "/proc/asound".to_string(),
        "/proc/kcore".to_string(),
        "/proc/keys".to_string(),
        "/proc/latency_stats".to_string(),
        "/proc/timer_list".to_string(),
        "/proc/timer_stats".to_string(),
        "/proc/sched_debug".to_string(),
        "/sys/firmware".to_string(),
        "/proc/scsi".to_string(),
    ]
}

// Default readonly paths,
// For example most containers shouldn't have permission to write to /proc/sys
pub fn get_default_readonly_paths() -> Vec<String> {
    vec![
        "/proc/bus".to_string(),
        "/proc/fs".to_string(),
        "/proc/irq".to_string(),
        "/proc/sys".to_string(),
        "/proc/sysrq-trigger".to_string(),
    ]
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FreezerState {
    Undefined,
    Frozen,
    Thawed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// LinuxIntelRdt has container runtime resource constraints for Intel RDT CAT and MBA features
/// which introduced in Linux 4.10 and 4.12 kernel.
pub struct LinuxIntelRdt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The identity for RDT Class of Service.
    pub clos_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The schema for L3 cache id and capacity bitmask (CBM).
    /// Format: "L3:<cache_id0>=<cbm0>;<cache_id1>=<cbm1>;..."
    pub l3_cache_schema: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The schema of memory bandwidth per L3 cache id.
    /// Format: "MB:<cache_id0>=bandwidth0;<cache_id1>=bandwidth1;..."
    /// The unit of memory bandwidth is specified in "percentages" by default, and in "MBps" if MBA
    /// Software Controller is enabled.
    pub mem_bw_schema: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxPersonality represents the Linux personality syscall input.
pub struct LinuxPersonality {
    /// Domain for the personality.
    domain: LinuxPersonalityDomain,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Additional flags
    flags: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// Define domain and flags for LinuxPersonality.
pub enum LinuxPersonalityDomain {
    #[serde(rename = "LINUX")]
    /// PerLinux is the standard Linux personality.
    PerLinux,

    #[serde(rename = "LINUX32")]
    /// PerLinux32 sets personality to 32 bit.
    PerLinux32,
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
            typ: typ.into(),
            major: some_none_generator_util::<i64>(g),
            minor: some_none_generator_util::<i64>(g),
            access: g.choose(&access_choices).unwrap().to_string().into(),
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
            disable_oom_killer: some_none_generator_util::<bool>(g),
            use_hierarchy: some_none_generator_util::<bool>(g),
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
