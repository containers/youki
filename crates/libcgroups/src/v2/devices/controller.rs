use std::os::unix::io::AsRawFd;
use std::path::Path;

use anyhow::Result;

use super::*;
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use oci_spec::runtime::LinuxDeviceCgroup;

use crate::common::{default_allow_devices, default_devices, ControllerOpt};
use crate::v2::controller::Controller;

#[cfg(test)]
use bpf::mock_prog as bpf_prog;

#[cfg(not(test))]
use bpf::prog as bpf_prog;

const LICENSE: &str = "Apache";

pub struct Devices {}

impl Controller for Devices {
    type Error = anyhow::Error;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        #[cfg(not(feature = "cgroupsv2_devices"))]
        return Ok(());

        #[cfg(feature = "cgroupsv2_devices")]
        return Self::apply_devices(cgroup_root, controller_opt.resources.devices());
    }
}

impl Devices {
    pub fn apply_devices(
        cgroup_root: &Path,
        linux_devices: &Option<Vec<LinuxDeviceCgroup>>,
    ) -> Result<()> {
        log::debug!("Apply Devices cgroup config");

        // FIXME: should we start as "deny all"?
        let mut emulator = emulator::Emulator::with_default_allow(false);

        // FIXME: apply user-defined and default rules in which order?
        if let Some(devices) = linux_devices {
            for d in devices {
                log::debug!("apply user defined rule: {:?}", d);
                emulator.add_rule(d)?;
            }
        }

        for d in [
            default_devices().iter().map(|d| d.into()).collect(),
            default_allow_devices(),
        ]
        .concat()
        {
            log::debug!("apply default rule: {:?}", d);
            emulator.add_rule(&d)?;
        }

        let prog = program::Program::from_rules(&emulator.rules, emulator.default_allow)?;

        // Increase `ulimit -l` limit to avoid BPF_PROG_LOAD error (#2167).
        // This limit is not inherited into the container.
        bpf_prog::bump_memlock_rlimit()?;
        let prog_fd = bpf_prog::load(LICENSE, prog.bytecodes())?;

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

        // get the fd of the cgroup root
        let fd = nix::dir::Dir::open(
            cgroup_root.as_os_str(),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY,
            Mode::from_bits(0o600).unwrap(),
        )?;

        // collect the programs attached to this cgroup
        let old_progs = bpf_prog::query(fd.as_raw_fd())?;
        // attach our new program
        bpf_prog::attach(prog_fd, fd.as_raw_fd())?;
        // detach all previous programs
        for old_prog in old_progs {
            bpf_prog::detach2(old_prog.fd, fd.as_raw_fd())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::setup;
    use serial_test::serial;

    use oci_spec::runtime::{LinuxDeviceCgroupBuilder, LinuxDeviceType};
    use std::os::unix::io::RawFd;

    use bpf::mock_prog;

    #[test]
    #[serial(bpf)] // mock contexts are shared
    fn test_apply_devices() {
        // arrange
        let (tmp, _) = setup("test_apply_devices", "some.value");
        let a_type = LinuxDeviceCgroupBuilder::default()
            .typ(LinuxDeviceType::A)
            .build()
            .unwrap();
        let file_descriptor: RawFd = 6;

        // expect
        let bump_memlock_rlimit = mock_prog::bump_memlock_rlimit_context();
        let load = mock_prog::load_context();
        let query = mock_prog::query_context();
        let attach = mock_prog::attach_context();
        let detach2 = mock_prog::detach2_context();
        bump_memlock_rlimit.expect().once().returning(|| Ok(()));
        load.expect()
            .once()
            .returning(move |_, _| Ok(file_descriptor));
        query.expect().once().returning(|_| Ok(vec![]));
        attach.expect().once().returning(|_, _| Ok(()));
        detach2.expect().never();

        // act
        Devices::apply_devices(&tmp, &Some(vec![a_type])).expect("Could not apply devices");
    }

    #[test]
    #[serial(bpf)] // mock contexts are shared
    fn test_existing_programs() {
        // arrange
        let (tmp, _) = setup("test_existing_programs", "some.value");
        let a_type = LinuxDeviceCgroupBuilder::default()
            .typ(LinuxDeviceType::A)
            .build()
            .unwrap();
        let file_descriptor: RawFd = 6;
        let existing_program_1 = bpf::ProgramInfo {
            id: u32::default(),
            fd: i32::default(),
        };

        // expect
        let bump_memlock_rlimit = mock_prog::bump_memlock_rlimit_context();
        let load = mock_prog::load_context();
        let query = mock_prog::query_context();
        let attach = mock_prog::attach_context();
        let detach2 = mock_prog::detach2_context();
        bump_memlock_rlimit.expect().once().returning(|| Ok(()));
        load.expect()
            .once()
            .returning(move |_, _| Ok(file_descriptor));
        query
            .expect()
            .once()
            .returning(move |_| Ok(vec![existing_program_1.clone()]));
        attach.expect().once().returning(|_, _| Ok(()));
        detach2.expect().once().returning(|_, _| Ok(()));

        // act
        Devices::apply_devices(&tmp, &Some(vec![a_type])).expect("Could not apply devices");
    }
}
