use super::*;
use nix::sys::stat::SFlag;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Linux {
    // UIDMapping for supporting user namespaces
    #[serde(default, rename = "uidMappings")]
    pub uid_mappings: Vec<LinuxIdMapping>,
    // GIDMapping for supporting group namespaces
    #[serde(default, rename = "gidMappings")]
    pub gid_mappings: Vec<LinuxIdMapping>,
    // Sysctl that are set for container on start
    #[serde(default)]
    pub sysctl: HashMap<String, String>,
    // Resources contain cgroup info for handling resource constraints
    #[serde(default)]
    pub resources: Option<LinuxResources>,
    // CgroupsPath specifies the path to cgroups that are created and/or joined by the container.
    // The path is expected to be relative to the cgroups mountpoint.
    // If resources are specified, the cgroups at CgroupsPath will be updated based on resources.
    #[serde(default)]
    pub cgroups_path: Option<PathBuf>,
    // Namespaces contains the namespaces that are created and/or joined by the container
    #[serde(default)]
    pub namespaces: Vec<LinuxNamespace>,
    // Devices are a list of device nodes that are created for the container
    #[serde(default)]
    pub devices: Vec<LinuxDevice>,
    // The rootfs mount propagation mode for the container
    #[serde(default)]
    pub rootfs_propagation: String,
    // Masks over the provided paths inside the container so they cannot be read
    #[serde(default)]
    pub masked_paths: Vec<String>,
    // Sets the provided paths as RO inside the container
    #[serde(default)]
    pub readonly_paths: Vec<String>,
    // Specifies th selinux context for the mounts in the container
    #[serde(default)]
    pub mount_label: String,
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
                    access: "rwm".to_string(),
                    allow: false,
                    typ: Default::default(),
                    major: Default::default(),
                    minor: Default::default(),
                }],
                disable_oom_killer: Default::default(),
                oom_score_adj: Default::default(),
                memory: Default::default(),
                cpu: Default::default(),
                pids: Default::default(),
                block_io: Default::default(),
                hugepage_limits: Default::default(),
                network: Default::default(),
                freezer: Default::default(),
            }),
            // Defaults to None
            cgroups_path: Default::default(),
            namespaces: get_default_namespaces(),
            // Empty Vec
            devices: Default::default(),
            // Empty String
            rootfs_propagation: Default::default(),
            masked_paths: get_default_maskedpaths(),
            readonly_paths: get_default_readonly_paths(),
            // Empty String
            mount_label: Default::default(),
        }
    }
}

// Specifies UID/GID mappings
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxIdMapping {
    // Starting uid/gid on the host to be mapped to container_id
    #[serde(default, rename = "hostID")]
    pub host_id: u32,
    // Starting uid/gid in the container
    #[serde(default, rename = "containerID")]
    pub container_id: u32,
    // Number of IDs to be mapped
    #[serde(default)]
    pub size: u32,
}

// Device types
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LinuxDeviceType {
    // block (buffered)
    B,
    // character (unbuffered)
    C,
    // character (unbufferd)
    U,
    // FIFO
    P,
    // ??
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

// Represents a device rule for the devices specified to the device controller
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct LinuxDeviceCgroup {
    // allow or deny
    #[serde(default)]
    pub allow: bool,
    // Device type, block, char, etc.
    #[serde(default, rename = "type")]
    pub typ: LinuxDeviceType,
    // Device's major number
    pub major: Option<i64>,
    // Device's minor number
    pub minor: Option<i64>,
    // Cgroup access premissions format, rwm.
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinuxMemory {
    pub limit: Option<i64>,
    pub reservation: Option<i64>,
    pub swap: Option<i64>,
    pub kernel: Option<i64>,
    #[serde(rename = "kernelTCP")]
    pub kernel_tcp: Option<i64>,
    pub swappiness: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinuxPids {
    #[serde(default)]
    pub limit: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxWeightDevice {
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    pub weight: Option<u16>,
    pub leaf_weight: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinuxThrottleDevice {
    #[serde(default)]
    pub major: i64,
    #[serde(default)]
    pub minor: i64,
    #[serde(default)]
    pub rate: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxHugepageLimit {
    #[serde(default)]
    pub page_size: String,
    #[serde(default)]
    pub limit: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxNetwork {
    #[serde(rename = "classID")]
    pub class_id: Option<u32>,
    #[serde(default)]
    pub priorities: Vec<LinuxInterfacePriority>,
}

// Resource constraints for container
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxResources {
    // Devices configures the device allow list
    #[serde(default)]
    pub devices: Vec<LinuxDeviceCgroup>,
    // Disables the OOM killer for out of memory conditions
    #[serde(default)]
    pub disable_oom_killer: bool,
    // Specify an oom_score_adj for container
    pub oom_score_adj: Option<i32>,
    // Memory usage restrictions
    pub memory: Option<LinuxMemory>,
    // CPU resource restrictions
    pub cpu: Option<LinuxCpu>,
    // Task resource restrictions
    pub pids: Option<LinuxPids>,
    // BlockIO restrictions
    #[serde(rename = "blockIO")]
    pub block_io: Option<LinuxBlockIo>,
    // Hugelb restrictions
    #[serde(default)]
    pub hugepage_limits: Vec<LinuxHugepageLimit>,
    // Network usage restrictions
    pub network: Option<LinuxNetwork>,
    pub freezer: Option<FreezerState>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LinuxNamespaceType {
    // Mount Namespace for isolating mount points
    Mount = 0x00020000,
    // Cgroup Namespace for isolating cgroup hierarchies
    Cgroup = 0x02000000,
    // Uts Namespace for isolating hostname and NIS domain name
    Uts = 0x04000000,
    // Ipc Namespace for isolating System V, IPC, POSIX message queues
    Ipc = 0x08000000,
    // User Namespace for isolating user and group  ids
    User = 0x10000000,
    // PID Namespace for isolating process ids
    Pid = 0x20000000,
    // Network Namespace for isolating network devices, ports, stacks etc.
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinuxNamespace {
    #[serde(rename = "type")]
    pub typ: LinuxNamespaceType,
    pub path: Option<PathBuf>,
}

// Utility function to get default namespaces
pub fn get_default_namespaces() -> Vec<LinuxNamespace> {
    let mut default_namespace = Vec::new();
    default_namespace.push(LinuxNamespace {
        typ: LinuxNamespaceType::Pid,
        path: Default::default(),
    });
    default_namespace.push(LinuxNamespace {
        typ: LinuxNamespaceType::Network,
        path: Default::default(),
    });
    default_namespace.push(LinuxNamespace {
        typ: LinuxNamespaceType::Ipc,
        path: Default::default(),
    });
    default_namespace.push(LinuxNamespace {
        typ: LinuxNamespaceType::Uts,
        path: Default::default(),
    });
    default_namespace.push(LinuxNamespace {
        typ: LinuxNamespaceType::Mount,
        path: Default::default(),
    });
    default_namespace
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
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
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezerState {
    Undefined,
    Frozen,
    Thawed,
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
