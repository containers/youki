use super::*;

// Specifies the container process. This property is used when youki start is called.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    // Creates interactive terminal for container
    #[serde(default)]
    pub terminal: bool,
    // Specifies size of console
    #[serde(default)]
    pub console_size: Box,
    // User info for process
    pub user: User,
    // Specifies the binary and arguments for the application to execute
    pub args: Vec<String>,
    // Populates the process enviroment
    #[serde(default)]
    pub env: Vec<String>,
    // current working directory relative to container root
    #[serde(default)]
    pub cwd: String,
    // Determines whether additional privileges can be gained by process
    #[serde(default)]
    pub no_new_privileges: bool,
    // Apparmor profile for the container
    #[serde(default)]
    pub apparmor_profile: String,
    // Selinux context that the container is run as
    #[serde(default)]
    pub selinux_label: String,
    // Linux capabilities that are kept for the process
    #[serde(default)]
    pub capabilities: Option<LinuxCapabilities>,
    // RLIMIT options to apply to the process
    #[serde(default)]
    pub rlimits: Vec<LinuxRlimit>,
}

// Default impl for processes in the container
impl Default for Process {
    fn default() -> Self {
        Process {
            // Creates an interactive terminal for container by default
            terminal: true,
            // Gives default console size of 0, 0
            console_size: Default::default(),
            // Gives process a uid and gid of 0 (root)
            user: Default::default(),
            // By default executes sh command, giving user shell
            args: vec![String::from("sh")],
            // Sets linux default enviroment for binaries and default xterm emulator
            env: vec![
                String::from("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"),
                String::from("TERM=xterm"),
            ],
            // Sets cwd of process to the container root by default
            cwd: String::from("/"),
            // By default does not allow process to gain additional privileges
            no_new_privileges: true,
            // Empty String, no default apparmor
            apparmor_profile: Default::default(),
            // Empty String, no default selinux
            selinux_label: Default::default(),
            // See impl Default for LinuxCapabilities
            capabilities: Some(Default::default()),
            // Sets the default maximum of 1024 files the process can open
            // This is the same as the linux kernel default
            rlimits: vec![LinuxRlimit {
                typ: LinuxRlimitType::RlimitNofile,
                hard: 1024,
                soft: 1024,
            }],
        }
    }
}

// Specifies the size of console
#[derive(Default, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Box {
    #[serde(default)]
    pub height: u64,
    #[serde(default)]
    pub width: u64,
}
// RLimit types available in youki (see https://man7.org/linux/man-pages/man2/getrlimit.2.html)
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LinuxRlimitType {
    // Limit in seconds of the amount of CPU time that the process can consume
    RlimitCpu,
    // Maximum size in bytes of the files that the process creates
    RlimitFsize,
    // Maximum size of the process's data segment (init data, uninit data and heap) in bytes
    RlimitData,
    // Maximum size of the proces stack in bytes
    RlimitStack,
    // Maximum size of a core dump file in bytes
    RlimitCore,
    // Limit on the process's resident set (the number of virtual pages resident in RAM)
    RlimitRss,
    // Limit on number of threads for the real uid calling processes
    RlimitNproc,
    // One greator than the maximum number of file descritors that one process may open
    RlimitNofile,
    // Maximum number of bytes of memory that may be locked into RAM
    RlimitMemlock,
    // Maximum size of the process's virtual memory(address space) in bytes
    RlimitAs,
    // Limit on the number of locks and leases for the process
    RlimitLocks,
    // Limit on number of signals that may be queued for the process
    RlimitSigpending,
    // Limit on the number of bytes that can be allocated for POSIX message queue
    RlimitMsgqueue,
    // Specifies a ceiling to which the process's nice value can be raised
    RlimitNice,
    // Specifies a ceiling on the real-time priority
    RlimitRtprio,
    // This is a limit (in microseconds) on the amount of CPU time
    // that a process scheduled under a real-time scheduling
    // policy may consume without making a blocking system call
    RlimitRttime,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinuxRlimit {
    // Type of Rlimit to set
    #[serde(rename = "type")]
    pub typ: LinuxRlimitType,
    // Hard limit for specified type
    #[serde(default)]
    pub hard: u64,
    // Soft limit for specified type
    #[serde(default)]
    pub soft: u64,
}

// user id (uid) and group id (gid) tracks file permssions
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    // user id
    #[serde(default)]
    pub uid: u32,
    // group id
    #[serde(default)]
    pub gid: u32,
    // additional group ids set for the container's process
    #[serde(default)]
    pub additional_gids: Vec<u32>,
    //user name
    #[serde(default)]
    pub username: String,
}

// Linux capabilities (see https://man7.org/linux/man-pages/man7/capabilities.7.html)
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum LinuxCapabilityType {
    CAP_CHOWN,
    CAP_DAC_OVERRIDE,
    CAP_DAC_READ_SEARCH,
    CAP_FOWNER,
    CAP_FSETID,
    CAP_KILL,
    CAP_SETGID,
    CAP_SETUID,
    CAP_SETPCAP,
    CAP_LINUX_IMMUTABLE,
    CAP_NET_BIND_SERVICE,
    CAP_NET_BROADCAST,
    CAP_NET_ADMIN,
    CAP_NET_RAW,
    CAP_IPC_LOCK,
    CAP_IPC_OWNER,
    CAP_SYS_MODULE,
    CAP_SYS_RAWIO,
    CAP_SYS_CHROOT,
    CAP_SYS_PTRACE,
    CAP_SYS_PACCT,
    CAP_SYS_ADMIN,
    CAP_SYS_BOOT,
    CAP_SYS_NICE,
    CAP_SYS_RESOURCE,
    CAP_SYS_TIME,
    CAP_SYS_TTY_CONFIG,
    CAP_MKNOD,
    CAP_LEASE,
    CAP_AUDIT_WRITE,
    CAP_AUDIT_CONTROL,
    CAP_SETFCAP,
    CAP_MAC_OVERRIDE,
    CAP_MAC_ADMIN,
    CAP_SYSLOG,
    CAP_WAKE_ALARM,
    CAP_BLOCK_SUSPEND,
    CAP_AUDIT_READ,
    CAP_PERFMON,
    CAP_BPF,
    CAP_CHECKPOINT_RESTORE,
}

// impl Into and From for LinuxCapabilityType
impl From<Capability> for LinuxCapabilityType {
    fn from(cap: Capability) -> Self {
        match cap {
            Capability::CAP_CHOWN => LinuxCapabilityType::CAP_CHOWN,
            Capability::CAP_DAC_OVERRIDE => LinuxCapabilityType::CAP_DAC_OVERRIDE,
            Capability::CAP_DAC_READ_SEARCH => LinuxCapabilityType::CAP_DAC_READ_SEARCH,
            Capability::CAP_FOWNER => LinuxCapabilityType::CAP_FOWNER,
            Capability::CAP_FSETID => LinuxCapabilityType::CAP_FSETID,
            Capability::CAP_KILL => LinuxCapabilityType::CAP_KILL,
            Capability::CAP_SETGID => LinuxCapabilityType::CAP_SETGID,
            Capability::CAP_SETUID => LinuxCapabilityType::CAP_SETUID,
            Capability::CAP_SETPCAP => LinuxCapabilityType::CAP_SETPCAP,
            Capability::CAP_LINUX_IMMUTABLE => LinuxCapabilityType::CAP_LINUX_IMMUTABLE,
            Capability::CAP_NET_BIND_SERVICE => LinuxCapabilityType::CAP_NET_BIND_SERVICE,
            Capability::CAP_NET_BROADCAST => LinuxCapabilityType::CAP_NET_BROADCAST,
            Capability::CAP_NET_ADMIN => LinuxCapabilityType::CAP_NET_ADMIN,
            Capability::CAP_NET_RAW => LinuxCapabilityType::CAP_NET_RAW,
            Capability::CAP_IPC_LOCK => LinuxCapabilityType::CAP_IPC_LOCK,
            Capability::CAP_IPC_OWNER => LinuxCapabilityType::CAP_IPC_OWNER,
            Capability::CAP_SYS_MODULE => LinuxCapabilityType::CAP_SYS_MODULE,
            Capability::CAP_SYS_RAWIO => LinuxCapabilityType::CAP_SYS_RAWIO,
            Capability::CAP_SYS_CHROOT => LinuxCapabilityType::CAP_SYS_CHROOT,
            Capability::CAP_SYS_PTRACE => LinuxCapabilityType::CAP_SYS_PTRACE,
            Capability::CAP_SYS_PACCT => LinuxCapabilityType::CAP_SYS_PACCT,
            Capability::CAP_SYS_ADMIN => LinuxCapabilityType::CAP_SYS_ADMIN,
            Capability::CAP_SYS_BOOT => LinuxCapabilityType::CAP_SYS_BOOT,
            Capability::CAP_SYS_NICE => LinuxCapabilityType::CAP_SYS_NICE,
            Capability::CAP_SYS_RESOURCE => LinuxCapabilityType::CAP_SYS_RESOURCE,
            Capability::CAP_SYS_TIME => LinuxCapabilityType::CAP_SYS_TIME,
            Capability::CAP_SYS_TTY_CONFIG => LinuxCapabilityType::CAP_SYS_TTY_CONFIG,
            Capability::CAP_SYSLOG => LinuxCapabilityType::CAP_SYSLOG,
            Capability::CAP_MKNOD => LinuxCapabilityType::CAP_MKNOD,
            Capability::CAP_LEASE => LinuxCapabilityType::CAP_LEASE,
            Capability::CAP_AUDIT_WRITE => LinuxCapabilityType::CAP_AUDIT_WRITE,
            Capability::CAP_AUDIT_CONTROL => LinuxCapabilityType::CAP_AUDIT_CONTROL,
            Capability::CAP_AUDIT_READ => LinuxCapabilityType::CAP_AUDIT_READ,
            Capability::CAP_SETFCAP => LinuxCapabilityType::CAP_SETFCAP,
            Capability::CAP_MAC_OVERRIDE => LinuxCapabilityType::CAP_MAC_OVERRIDE,
            Capability::CAP_MAC_ADMIN => LinuxCapabilityType::CAP_MAC_ADMIN,
            Capability::CAP_WAKE_ALARM => LinuxCapabilityType::CAP_WAKE_ALARM,
            Capability::CAP_BLOCK_SUSPEND => LinuxCapabilityType::CAP_BLOCK_SUSPEND,
            Capability::CAP_PERFMON => LinuxCapabilityType::CAP_PERFMON,
            Capability::CAP_BPF => LinuxCapabilityType::CAP_BPF,
            Capability::CAP_CHECKPOINT_RESTORE => LinuxCapabilityType::CAP_CHECKPOINT_RESTORE,
            Capability::__Nonexhaustive => unreachable!("unexpected Linux Capability Type"),
        }
    }
}

// impl Into and From for caps::Capability
impl From<LinuxCapabilityType> for Capability {
    fn from(linux_cap: LinuxCapabilityType) -> Self {
        match linux_cap {
            LinuxCapabilityType::CAP_CHOWN => Capability::CAP_CHOWN,
            LinuxCapabilityType::CAP_DAC_OVERRIDE => Capability::CAP_DAC_OVERRIDE,
            LinuxCapabilityType::CAP_DAC_READ_SEARCH => Capability::CAP_DAC_READ_SEARCH,
            LinuxCapabilityType::CAP_FOWNER => Capability::CAP_FOWNER,
            LinuxCapabilityType::CAP_FSETID => Capability::CAP_FSETID,
            LinuxCapabilityType::CAP_KILL => Capability::CAP_KILL,
            LinuxCapabilityType::CAP_SETGID => Capability::CAP_SETGID,
            LinuxCapabilityType::CAP_SETUID => Capability::CAP_SETUID,
            LinuxCapabilityType::CAP_SETPCAP => Capability::CAP_SETPCAP,
            LinuxCapabilityType::CAP_LINUX_IMMUTABLE => Capability::CAP_LINUX_IMMUTABLE,
            LinuxCapabilityType::CAP_NET_BIND_SERVICE => Capability::CAP_NET_BIND_SERVICE,
            LinuxCapabilityType::CAP_NET_BROADCAST => Capability::CAP_NET_BROADCAST,
            LinuxCapabilityType::CAP_NET_ADMIN => Capability::CAP_NET_ADMIN,
            LinuxCapabilityType::CAP_NET_RAW => Capability::CAP_NET_RAW,
            LinuxCapabilityType::CAP_IPC_LOCK => Capability::CAP_IPC_LOCK,
            LinuxCapabilityType::CAP_IPC_OWNER => Capability::CAP_IPC_OWNER,
            LinuxCapabilityType::CAP_SYS_MODULE => Capability::CAP_SYS_MODULE,
            LinuxCapabilityType::CAP_SYS_RAWIO => Capability::CAP_SYS_RAWIO,
            LinuxCapabilityType::CAP_SYS_CHROOT => Capability::CAP_SYS_CHROOT,
            LinuxCapabilityType::CAP_SYS_PTRACE => Capability::CAP_SYS_PTRACE,
            LinuxCapabilityType::CAP_SYS_PACCT => Capability::CAP_SYS_PACCT,
            LinuxCapabilityType::CAP_SYS_ADMIN => Capability::CAP_SYS_ADMIN,
            LinuxCapabilityType::CAP_SYS_BOOT => Capability::CAP_SYS_BOOT,
            LinuxCapabilityType::CAP_SYS_NICE => Capability::CAP_SYS_NICE,
            LinuxCapabilityType::CAP_SYS_RESOURCE => Capability::CAP_SYS_RESOURCE,
            LinuxCapabilityType::CAP_SYS_TIME => Capability::CAP_SYS_TIME,
            LinuxCapabilityType::CAP_SYS_TTY_CONFIG => Capability::CAP_SYS_TTY_CONFIG,
            LinuxCapabilityType::CAP_SYSLOG => Capability::CAP_SYSLOG,
            LinuxCapabilityType::CAP_MKNOD => Capability::CAP_MKNOD,
            LinuxCapabilityType::CAP_LEASE => Capability::CAP_LEASE,
            LinuxCapabilityType::CAP_AUDIT_WRITE => Capability::CAP_AUDIT_WRITE,
            LinuxCapabilityType::CAP_AUDIT_CONTROL => Capability::CAP_AUDIT_CONTROL,
            LinuxCapabilityType::CAP_AUDIT_READ => Capability::CAP_AUDIT_READ,
            LinuxCapabilityType::CAP_SETFCAP => Capability::CAP_SETFCAP,
            LinuxCapabilityType::CAP_MAC_OVERRIDE => Capability::CAP_MAC_OVERRIDE,
            LinuxCapabilityType::CAP_MAC_ADMIN => Capability::CAP_MAC_ADMIN,
            LinuxCapabilityType::CAP_WAKE_ALARM => Capability::CAP_WAKE_ALARM,
            LinuxCapabilityType::CAP_BLOCK_SUSPEND => Capability::CAP_BLOCK_SUSPEND,
            LinuxCapabilityType::CAP_PERFMON => Capability::CAP_PERFMON,
            LinuxCapabilityType::CAP_BPF => Capability::CAP_BPF,
            LinuxCapabilityType::CAP_CHECKPOINT_RESTORE => Capability::CAP_CHECKPOINT_RESTORE,
        }
    }
}

// see https://man7.org/linux/man-pages/man7/capabilities.7.html
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinuxCapabilities {
    // Limiting superset for capabilities that can be added to the inheritable set (for security)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub bounding: Vec<LinuxCapabilityType>,
    // Capability set used by kernel to perform permission checks for container process
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub effective: Vec<LinuxCapabilityType>,
    // set of capabilities preserved across an execve(2)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inheritable: Vec<LinuxCapabilityType>,
    // Limiting superset for the effective capabilities that the container may assume
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permitted: Vec<LinuxCapabilityType>,
    // set of capabilities preserved across non root execve(2),
    // capabilities must be both permitted and inheritable to be ambient
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ambient: Vec<LinuxCapabilityType>,
}

// Default container's linux capabilities:
// CAP_AUDIT_WRITE gives container ability to write to linux audit logs,
// CAP_KILL gives container ability to kill non root processes
// CAP_NET_BIND_SERVICE allows container to bind to ports below 1024
impl Default for LinuxCapabilities {
    fn default() -> Self {
        let audit_write = LinuxCapabilityType::CAP_AUDIT_WRITE;
        let cap_kill = LinuxCapabilityType::CAP_KILL;
        let net_bind = LinuxCapabilityType::CAP_NET_BIND_SERVICE;
        let default_vec = vec![audit_write, cap_kill, net_bind];
        LinuxCapabilities {
            bounding: default_vec.clone(),
            effective: default_vec.clone(),
            inheritable: default_vec.clone(),
            permitted: default_vec.clone(),
            ambient: default_vec.clone(),
        }
    }
}
