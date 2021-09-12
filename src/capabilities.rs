//! Handles Management of Capabilities
use crate::syscall::Syscall;
use caps::Capability as CapsCapability;
use caps::*;

use anyhow::Result;
use oci_spec::runtime::{Capabilities, Capability as SpecCapability, LinuxCapabilities};

/// Converts a list of capability types to capabilities has set
fn to_set(caps: &Capabilities) -> CapsHashSet {
    let mut capabilities = CapsHashSet::new();

    for c in caps {
        let cap = c.to_cap();
        capabilities.insert(cap);
    }
    capabilities
}

pub trait CapabilityExt {
    /// Convert self to caps::Capability
    fn to_cap(&self) -> caps::Capability;
    /// Convert caps::Capability to self
    fn from_cap(c: CapsCapability) -> Self;
}

impl CapabilityExt for SpecCapability {
    /// Convert oci::runtime::Capability to caps::Capability
    fn to_cap(&self) -> caps::Capability {
        match self {
            SpecCapability::AuditControl => CapsCapability::CAP_AUDIT_CONTROL,
            SpecCapability::AuditRead => CapsCapability::CAP_AUDIT_READ,
            SpecCapability::AuditWrite => CapsCapability::CAP_AUDIT_WRITE,
            SpecCapability::BlockSuspend => CapsCapability::CAP_BLOCK_SUSPEND,
            SpecCapability::Bpf => CapsCapability::CAP_BPF,
            SpecCapability::CheckpointRestore => CapsCapability::CAP_CHECKPOINT_RESTORE,
            SpecCapability::Chown => CapsCapability::CAP_CHOWN,
            SpecCapability::DacOverride => CapsCapability::CAP_DAC_OVERRIDE,
            SpecCapability::DacReadSearch => CapsCapability::CAP_DAC_READ_SEARCH,
            SpecCapability::Fowner => CapsCapability::CAP_FOWNER,
            SpecCapability::Fsetid => CapsCapability::CAP_FSETID,
            SpecCapability::IpcLock => CapsCapability::CAP_IPC_LOCK,
            SpecCapability::IpcOwner => CapsCapability::CAP_IPC_OWNER,
            SpecCapability::Kill => CapsCapability::CAP_KILL,
            SpecCapability::Lease => CapsCapability::CAP_LEASE,
            SpecCapability::LinuxImmutable => CapsCapability::CAP_LINUX_IMMUTABLE,
            SpecCapability::MacAdmin => CapsCapability::CAP_MAC_ADMIN,
            SpecCapability::MacOverride => CapsCapability::CAP_MAC_OVERRIDE,
            SpecCapability::Mknod => CapsCapability::CAP_MKNOD,
            SpecCapability::NetAdmin => CapsCapability::CAP_NET_ADMIN,
            SpecCapability::NetBindService => CapsCapability::CAP_NET_BIND_SERVICE,
            SpecCapability::NetBroadcast => CapsCapability::CAP_NET_BROADCAST,
            SpecCapability::NetRaw => CapsCapability::CAP_NET_RAW,
            SpecCapability::Perfmon => CapsCapability::CAP_PERFMON,
            SpecCapability::Setgid => CapsCapability::CAP_SETGID,
            SpecCapability::Setfcap => CapsCapability::CAP_SETFCAP,
            SpecCapability::Setpcap => CapsCapability::CAP_SETPCAP,
            SpecCapability::Setuid => CapsCapability::CAP_SETUID,
            SpecCapability::SysAdmin => CapsCapability::CAP_SYS_ADMIN,
            SpecCapability::SysBoot => CapsCapability::CAP_SYS_BOOT,
            SpecCapability::SysChroot => CapsCapability::CAP_SYS_CHROOT,
            SpecCapability::SysModule => CapsCapability::CAP_SYS_MODULE,
            SpecCapability::SysNice => CapsCapability::CAP_SYS_NICE,
            SpecCapability::SysPacct => CapsCapability::CAP_SYS_PACCT,
            SpecCapability::SysPtrace => CapsCapability::CAP_SYS_PTRACE,
            SpecCapability::SysRawio => CapsCapability::CAP_SYS_RAWIO,
            SpecCapability::SysResource => CapsCapability::CAP_SYS_RESOURCE,
            SpecCapability::SysTime => CapsCapability::CAP_SYS_TIME,
            SpecCapability::SysTtyConfig => CapsCapability::CAP_SYS_TTY_CONFIG,
            SpecCapability::Syslog => CapsCapability::CAP_SYSLOG,
            SpecCapability::WakeAlarm => CapsCapability::CAP_WAKE_ALARM,
        }
    }

    /// Convert caps::Capability to oci::runtime::Capability
    fn from_cap(c: CapsCapability) -> SpecCapability {
        match c {
            CapsCapability::CAP_AUDIT_CONTROL => SpecCapability::AuditControl,
            CapsCapability::CAP_AUDIT_READ => SpecCapability::AuditRead,
            CapsCapability::CAP_AUDIT_WRITE => SpecCapability::AuditWrite,
            CapsCapability::CAP_BLOCK_SUSPEND => SpecCapability::BlockSuspend,
            CapsCapability::CAP_BPF => SpecCapability::Bpf,
            CapsCapability::CAP_CHECKPOINT_RESTORE => SpecCapability::CheckpointRestore,
            CapsCapability::CAP_CHOWN => SpecCapability::Chown,
            CapsCapability::CAP_DAC_OVERRIDE => SpecCapability::DacOverride,
            CapsCapability::CAP_DAC_READ_SEARCH => SpecCapability::DacReadSearch,
            CapsCapability::CAP_FOWNER => SpecCapability::Fowner,
            CapsCapability::CAP_FSETID => SpecCapability::Fsetid,
            CapsCapability::CAP_IPC_LOCK => SpecCapability::IpcLock,
            CapsCapability::CAP_IPC_OWNER => SpecCapability::IpcOwner,
            CapsCapability::CAP_KILL => SpecCapability::Kill,
            CapsCapability::CAP_LEASE => SpecCapability::Lease,
            CapsCapability::CAP_LINUX_IMMUTABLE => SpecCapability::LinuxImmutable,
            CapsCapability::CAP_MAC_ADMIN => SpecCapability::MacAdmin,
            CapsCapability::CAP_MAC_OVERRIDE => SpecCapability::MacOverride,
            CapsCapability::CAP_MKNOD => SpecCapability::Mknod,
            CapsCapability::CAP_NET_ADMIN => SpecCapability::NetAdmin,
            CapsCapability::CAP_NET_BIND_SERVICE => SpecCapability::NetBindService,
            CapsCapability::CAP_NET_BROADCAST => SpecCapability::NetBroadcast,
            CapsCapability::CAP_NET_RAW => SpecCapability::NetRaw,
            CapsCapability::CAP_PERFMON => SpecCapability::Perfmon,
            CapsCapability::CAP_SETGID => SpecCapability::Setgid,
            CapsCapability::CAP_SETFCAP => SpecCapability::Setfcap,
            CapsCapability::CAP_SETPCAP => SpecCapability::Setpcap,
            CapsCapability::CAP_SETUID => SpecCapability::Setuid,
            CapsCapability::CAP_SYS_ADMIN => SpecCapability::SysAdmin,
            CapsCapability::CAP_SYS_BOOT => SpecCapability::SysBoot,
            CapsCapability::CAP_SYS_CHROOT => SpecCapability::SysChroot,
            CapsCapability::CAP_SYS_MODULE => SpecCapability::SysModule,
            CapsCapability::CAP_SYS_NICE => SpecCapability::SysNice,
            CapsCapability::CAP_SYS_PACCT => SpecCapability::SysPacct,
            CapsCapability::CAP_SYS_PTRACE => SpecCapability::SysPtrace,
            CapsCapability::CAP_SYS_RAWIO => SpecCapability::SysRawio,
            CapsCapability::CAP_SYS_RESOURCE => SpecCapability::SysResource,
            CapsCapability::CAP_SYS_TIME => SpecCapability::SysTime,
            CapsCapability::CAP_SYS_TTY_CONFIG => SpecCapability::SysTtyConfig,
            CapsCapability::CAP_SYSLOG => SpecCapability::Syslog,
            CapsCapability::CAP_WAKE_ALARM => SpecCapability::WakeAlarm,
            CapsCapability::__Nonexhaustive => unreachable!("invalid capability"),
        }
    }
}

/// reset capabilities of process calling this to effective capabilities
/// effective capability set is set of capabilities used by kernel to perform checks
/// see https://man7.org/linux/man-pages/man7/capabilities.7.html for more information
pub fn reset_effective(syscall: &impl Syscall) -> Result<()> {
    log::debug!("reset all caps");
    syscall.set_capability(CapSet::Effective, &caps::all())?;
    Ok(())
}

/// Drop any extra granted capabilities, and reset to defaults which are in oci specification
pub fn drop_privileges(cs: &LinuxCapabilities, syscall: &impl Syscall) -> Result<()> {
    log::debug!("dropping bounding capabilities to {:?}", cs.bounding);
    if let Some(bounding) = cs.bounding.as_ref() {
        syscall.set_capability(CapSet::Bounding, &to_set(bounding))?;
    }

    if let Some(effective) = cs.effective.as_ref() {
        syscall.set_capability(CapSet::Effective, &to_set(effective))?;
    }

    if let Some(permitted) = cs.permitted.as_ref() {
        syscall.set_capability(CapSet::Permitted, &to_set(permitted))?;
    }

    if let Some(inheritable) = cs.inheritable.as_ref() {
        syscall.set_capability(CapSet::Inheritable, &to_set(inheritable))?;
    }

    if let Some(ambient) = cs.ambient.as_ref() {
        // check specifically for ambient, as those might not always be available
        if let Err(e) = syscall.set_capability(CapSet::Ambient, &to_set(ambient)) {
            log::error!("failed to set ambient capabilities: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::test::TestHelperSyscall;

    #[test]
    fn test_reset_effective() {
        let test_command = TestHelperSyscall::default();
        assert!(reset_effective(&test_command).is_ok());
        let set_capability_args: Vec<_> = test_command
            .get_set_capability_args()
            .into_iter()
            .map(|(_capset, caps)| caps)
            .collect();
        assert_eq!(set_capability_args, vec![caps::all()]);
    }

    #[test]
    fn test_convert_oci_spec_to_caps_type() {
        struct Testcase {
            want: CapsCapability,
            input: SpecCapability,
        }

        let tests = vec![
            Testcase {
                input: oci_spec::runtime::Capability::AuditControl,
                want: CapsCapability::CAP_AUDIT_CONTROL,
            },
            Testcase {
                input: oci_spec::runtime::Capability::AuditRead,
                want: CapsCapability::CAP_AUDIT_READ,
            },
            Testcase {
                input: oci_spec::runtime::Capability::AuditWrite,
                want: CapsCapability::CAP_AUDIT_WRITE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::BlockSuspend,
                want: CapsCapability::CAP_BLOCK_SUSPEND,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Bpf,
                want: CapsCapability::CAP_BPF,
            },
            Testcase {
                input: oci_spec::runtime::Capability::CheckpointRestore,
                want: CapsCapability::CAP_CHECKPOINT_RESTORE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Chown,
                want: Capability::CAP_CHOWN,
            },
            Testcase {
                input: oci_spec::runtime::Capability::DacOverride,
                want: CapsCapability::CAP_DAC_OVERRIDE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::DacReadSearch,
                want: CapsCapability::CAP_DAC_READ_SEARCH,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Fowner,
                want: CapsCapability::CAP_FOWNER,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Fsetid,
                want: CapsCapability::CAP_FSETID,
            },
            Testcase {
                input: oci_spec::runtime::Capability::IpcLock,
                want: CapsCapability::CAP_IPC_LOCK,
            },
            Testcase {
                input: oci_spec::runtime::Capability::IpcOwner,
                want: CapsCapability::CAP_IPC_OWNER,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Kill,
                want: CapsCapability::CAP_KILL,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Lease,
                want: CapsCapability::CAP_LEASE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::LinuxImmutable,
                want: CapsCapability::CAP_LINUX_IMMUTABLE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::MacAdmin,
                want: CapsCapability::CAP_MAC_ADMIN,
            },
            Testcase {
                input: oci_spec::runtime::Capability::MacOverride,
                want: CapsCapability::CAP_MAC_OVERRIDE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Mknod,
                want: CapsCapability::CAP_MKNOD,
            },
            Testcase {
                input: oci_spec::runtime::Capability::NetAdmin,
                want: CapsCapability::CAP_NET_ADMIN,
            },
            Testcase {
                input: oci_spec::runtime::Capability::NetBindService,
                want: CapsCapability::CAP_NET_BIND_SERVICE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::NetBroadcast,
                want: CapsCapability::CAP_NET_BROADCAST,
            },
            Testcase {
                input: oci_spec::runtime::Capability::NetRaw,
                want: CapsCapability::CAP_NET_RAW,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Perfmon,
                want: CapsCapability::CAP_PERFMON,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Setgid,
                want: CapsCapability::CAP_SETGID,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Setfcap,
                want: CapsCapability::CAP_SETFCAP,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Setpcap,
                want: CapsCapability::CAP_SETPCAP,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Setuid,
                want: CapsCapability::CAP_SETUID,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysAdmin,
                want: CapsCapability::CAP_SYS_ADMIN,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysBoot,
                want: CapsCapability::CAP_SYS_BOOT,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysChroot,
                want: CapsCapability::CAP_SYS_CHROOT,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysModule,
                want: CapsCapability::CAP_SYS_MODULE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysNice,
                want: CapsCapability::CAP_SYS_NICE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysPacct,
                want: CapsCapability::CAP_SYS_PACCT,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysPtrace,
                want: CapsCapability::CAP_SYS_PTRACE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysRawio,
                want: CapsCapability::CAP_SYS_RAWIO,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysResource,
                want: CapsCapability::CAP_SYS_RESOURCE,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysTime,
                want: CapsCapability::CAP_SYS_TIME,
            },
            Testcase {
                input: oci_spec::runtime::Capability::SysTtyConfig,
                want: CapsCapability::CAP_SYS_TTY_CONFIG,
            },
            Testcase {
                input: oci_spec::runtime::Capability::Syslog,
                want: CapsCapability::CAP_SYSLOG,
            },
            Testcase {
                input: oci_spec::runtime::Capability::WakeAlarm,
                want: CapsCapability::CAP_WAKE_ALARM,
            },
        ];

        for test in tests {
            let got = test.input.to_cap();
            assert_eq!(got, test.want);
        }
    }
}
