use caps::Capability;
use serde::{Deserialize, Serialize};

/// Process contains information to start a specific application inside the container.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Terminal creates an interactive terminal for the container.
    pub terminal: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// ConsoleSize specifies the size of the console.
    pub console_size: Option<Box>,

    /// User specifies user information for the process.
    pub user: User,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Args specifies the binary and arguments for the application to execute.
    pub args: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CommandLine specifies the full command line for the application to execute on Windows.
    pub command_line: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Env populates the process environment for the process.
    pub env: Option<Vec<String>>,

    /// Cwd is the current working directory for the process and must be relative to the
    /// container's root.
    pub cwd: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Capabilities are Linux capabilities that are kept for the process.
    pub capabilities: Option<LinuxCapabilities>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Rlimits specifies rlimit options to apply to the process.
    pub rlimits: Option<Vec<LinuxRlimit>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// NoNewPrivileges controls whether additional privileges could be gained by processes in the
    /// container.
    pub no_new_privileges: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// ApparmorProfile specifies the apparmor profile for the container.
    pub apparmor_profile: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Specify an oom_score_adj for the container.
    pub oom_score_adj: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// SelinuxLabel specifies the selinux context that the container process is run as.
    pub selinux_label: Option<String>,
}

// Default impl for processes in the container
impl Default for Process {
    fn default() -> Self {
        Process {
            // Creates an interactive terminal for container by default
            terminal: true.into(),
            // Gives default console size of 0, 0
            console_size: Default::default(),
            // Gives process a uid and gid of 0 (root)
            user: Default::default(),
            // By default executes sh command, giving user shell
            args: vec!["sh".to_string()].into(),
            // Sets linux default enviroment for binaries and default xterm emulator
            env: vec![
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
                "TERM=xterm".into(),
            ]
            .into(),
            // Sets cwd of process to the container root by default
            cwd: "/".into(),
            // By default does not allow process to gain additional privileges
            no_new_privileges: true.into(),
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
            }]
            .into(),
            oom_score_adj: None,
            command_line: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// Box specifies dimensions of a rectangle. Used for specifying the size of a console.
pub struct Box {
    #[serde(default)]
    /// Height is the vertical dimension of a box.
    pub height: u64,

    #[serde(default)]
    /// Width is the horizontal dimension of a box.
    pub width: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
/// Available rlimti types (see https://man7.org/linux/man-pages/man2/getrlimit.2.html)
pub enum LinuxRlimitType {
    /// Limit in seconds of the amount of CPU time that the process can consume.
    RlimitCpu,

    /// Maximum size in bytes of the files that the process creates.
    RlimitFsize,

    /// Maximum size of the process's data segment (init data, uninit data and heap) in bytes.
    RlimitData,

    /// Maximum size of the proces stack in bytes.
    RlimitStack,

    /// Maximum size of a core dump file in bytes.
    RlimitCore,

    /// Limit on the process's resident set (the number of virtual pages resident in RAM).
    RlimitRss,

    /// Limit on number of threads for the real uid calling processes.
    RlimitNproc,

    /// One greator than the maximum number of file descritors that one process may open.
    RlimitNofile,

    /// Maximum number of bytes of memory that may be locked into RAM.
    RlimitMemlock,

    /// Maximum size of the process's virtual memory(address space) in bytes.
    RlimitAs,

    /// Limit on the number of locks and leases for the process.
    RlimitLocks,

    /// Limit on number of signals that may be queued for the process.
    RlimitSigpending,

    /// Limit on the number of bytes that can be allocated for POSIX message queue.
    RlimitMsgqueue,

    /// Specifies a ceiling to which the process's nice value can be raised.
    RlimitNice,

    /// Specifies a ceiling on the real-time priority.
    RlimitRtprio,

    /// This is a limit (in microseconds) on the amount of CPU time that a process scheduled under
    /// a real-time scheduling policy may consume without making a blocking system call.
    RlimitRttime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// RLimit types and restrictions.
pub struct LinuxRlimit {
    #[serde(rename = "type")]
    /// Type of Rlimit to set
    pub typ: LinuxRlimitType,

    #[serde(default)]
    /// Hard limit for specified type
    pub hard: u64,

    #[serde(default)]
    /// Soft limit for specified type
    pub soft: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// User id (uid) and group id (gid) tracks file permssions.
pub struct User {
    #[serde(default)]
    /// UID is the user id.
    pub uid: u32,

    #[serde(default)]
    /// GID is the group id.
    pub gid: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// AdditionalGids are additional group ids set for the container's process.
    pub additional_gids: Option<Vec<u32>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Username is the user name.
    pub username: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// LinuxCapabilities specifies the list of allowed capabilities that are kept for a process.
/// http://man7.org/linux/man-pages/man7/capabilities.7.html
pub struct LinuxCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Bounding is the set of capabilities checked by the kernel.
    pub bounding: Option<Vec<Capability>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Effective is the set of capabilities checked by the kernel.
    pub effective: Option<Vec<Capability>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Inheritable is the capabilities preserved across execve.
    pub inheritable: Option<Vec<Capability>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Permitted is the limiting superset for effective capabilities.
    pub permitted: Option<Vec<Capability>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    //// Ambient is the ambient set of capabilities that are kept.
    pub ambient: Option<Vec<Capability>>,
}

// Default container's linux capabilities:
// CAP_AUDIT_WRITE gives container ability to write to linux audit logs,
// CAP_KILL gives container ability to kill non root processes
// CAP_NET_BIND_SERVICE allows container to bind to ports below 1024
impl Default for LinuxCapabilities {
    fn default() -> Self {
        let audit_write = Capability::CAP_AUDIT_WRITE;
        let cap_kill = Capability::CAP_KILL;
        let net_bind = Capability::CAP_NET_BIND_SERVICE;
        let default_vec = vec![audit_write, cap_kill, net_bind];
        LinuxCapabilities {
            bounding: default_vec.clone().into(),
            effective: default_vec.clone().into(),
            inheritable: default_vec.clone().into(),
            permitted: default_vec.clone().into(),
            ambient: default_vec.into(),
        }
    }
}
