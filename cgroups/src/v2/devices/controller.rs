use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::*;
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use oci_spec::{LinuxDevice, LinuxDeviceCgroup, LinuxDeviceType, LinuxResources};

pub struct Devices {}

const LICENSE: &'static str = &"Apache";

impl Devices {
    pub fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Devices cgroup config");

        // FIXME: should we start as "deny all"?
        let mut emulator = emulator::Emulator::with_default_allow(false);

        // FIXME: apply user-defined and default rules in which order?
        if let Some(devices) = linux_resources.devices.as_ref() {
            for d in devices {
                log::debug!("apply user defined rule: {:?}", d);
                emulator.add_rule(d)?;
            }
        }

        for d in [
            Self::default_devices().iter().map(|d| d.into()).collect(),
            Self::default_allow_devices(),
        ]
        .concat()
        {
            log::debug!("apply default rule: {:?}", d);
            emulator.add_rule(&d)?;
        }

        let prog = program::Program::from_rules(&emulator.rules, emulator.default_allow)?;

        // Increase `ulimit -l` limit to avoid BPF_PROG_LOAD error (#2167).
        // This limit is not inherited into the container.
        bpf::bump_memlock_rlimit()?;
        let prog_fd = bpf::prog_load(LICENSE, prog.bytecodes())?;

        // FIXME: simple way to attach BPF program
        //  1. get list of existing attached programs
        //  2. attach this program (not use BPF_F_REPLACE, see below)
        //  3. detach all programs of 1
        //
        // runc will use BPF_F_REPLACE to replace currently attached progam if:
        //   1. BPF_F_REPLACE is supported by kernel
        //   2. there is exactly one attached program
        // https://github.com/opencontainers/runc/blob/8e6871a3b14bb74e0ef358aca3b9f8f9cb80f041/libcontainer/cgroups/ebpf/ebpf_linux.go#L165
        //
        // IMHO, this is too complicated, and in most cases, we just attach program once without
        // already attached programs.

        let fd = nix::dir::Dir::open(
            cgroup_root.as_os_str(),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY,
            Mode::from_bits(0o600).unwrap(),
        )?;

        let old_progs = bpf::prog_query(fd.as_raw_fd())?;
        bpf::prog_attach(prog_fd, fd.as_raw_fd())?;
        for old_prog in old_progs {
            bpf::prog_detach2(old_prog.fd, fd.as_raw_fd())?;
        }

        Ok(())
    }
    // FIXME: move to common
    fn default_allow_devices() -> Vec<LinuxDeviceCgroup> {
        vec![
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: None,
                minor: None,
                access: "m".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::B),
                major: None,
                minor: None,
                access: "m".to_string().into(),
            },
            // /dev/console
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(5),
                minor: Some(1),
                access: "rwm".to_string().into(),
            },
            // /dev/pts
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(136),
                minor: None,
                access: "rwm".to_string().into(),
            },
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(5),
                minor: Some(2),
                access: "rwm".to_string().into(),
            },
            // tun/tap
            LinuxDeviceCgroup {
                allow: true,
                typ: Some(LinuxDeviceType::C),
                major: Some(10),
                minor: Some(200),
                access: "rwm".to_string().into(),
            },
        ]
    }

    pub fn default_devices() -> Vec<LinuxDevice> {
        vec![
            LinuxDevice {
                path: PathBuf::from("/dev/null"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 3,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/zero"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 5,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/full"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 7,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/tty"),
                typ: LinuxDeviceType::C,
                major: 5,
                minor: 0,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/urandom"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 9,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
            LinuxDevice {
                path: PathBuf::from("/dev/random"),
                typ: LinuxDeviceType::C,
                major: 1,
                minor: 8,
                file_mode: Some(0o066),
                uid: None,
                gid: None,
            },
        ]
    }
}

// FIXME: add tests, but how to?
