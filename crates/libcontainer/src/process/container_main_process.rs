use crate::{
    container::ContainerProcessState,
    process::{
        args::ContainerArgs, channel, container_intermediate_process, fork,
        intel_rdt::setup_intel_rdt,
    },
    rootless::Rootless,
    utils,
};
use anyhow::{bail, Context, Result};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::Pid;

#[cfg(feature = "libseccomp")]
use crate::seccomp;
#[cfg(feature = "libseccomp")]
use nix::{
    sys::socket::{self, UnixAddr},
    unistd,
};
#[cfg(feature = "libseccomp")]
use oci_spec::runtime;
#[cfg(feature = "libseccomp")]
use std::{io::IoSlice, path::Path};

pub fn container_main_process(container_args: &ContainerArgs) -> Result<(Pid, bool)> {
    // We use a set of channels to communicate between parent and child process.
    // Each channel is uni-directional. Because we will pass these channel to
    // cloned process, we have to be deligent about closing any unused channel.
    // At minimum, we have to close down any unused senders. The corresponding
    // receivers will be cleaned up once the senders are closed down.
    let (main_sender, main_receiver) = &mut channel::main_channel()?;
    let inter_chan = &mut channel::intermediate_channel()?;
    let init_chan = &mut channel::init_channel()?;

    let intermediate_pid = fork::container_fork("youki:[1:INTER]", || {
        container_intermediate_process::container_intermediate_process(
            container_args,
            inter_chan,
            init_chan,
            main_sender,
        )?;

        Ok(0)
    })?;

    // Close down unused fds. The corresponding fds are duplicated to the
    // child process during fork.
    main_sender
        .close()
        .context("failed to close unused sender")?;

    let (inter_sender, _) = inter_chan;
    let (init_sender, _) = init_chan;

    // If creating a rootless container, the intermediate process will ask
    // the main process to set up uid and gid mapping, once the intermediate
    // process enters into a new user namespace.
    if let Some(rootless) = &container_args.rootless {
        main_receiver.wait_for_mapping_request()?;
        setup_mapping(rootless, intermediate_pid)?;
        inter_sender.mapping_written()?;
    }

    // At this point, we don't need to send any message to intermediate process anymore,
    // so we want to close this sender at the earliest point.
    inter_sender
        .close()
        .context("failed to close unused intermediate sender")?;

    // The intermediate process will send the init pid once it forks the init
    // process.  The intermediate process should exit after this point.
    let init_pid = main_receiver.wait_for_intermediate_ready()?;
    let mut need_to_clean_up_intel_rdt_subdirectory = false;

    if let Some(linux) = container_args.spec.linux() {
        if let Some(seccomp) = linux.seccomp() {
            #[allow(unused_variables)]
            let state = ContainerProcessState {
                oci_version: container_args.spec.version().to_string(),
                // runc hardcode the `seccompFd` name for fds.
                fds: vec![String::from("seccompFd")],
                pid: init_pid.as_raw(),
                metadata: seccomp.listener_metadata().to_owned().unwrap_or_default(),
                state: container_args
                    .container
                    .as_ref()
                    .context("container state is required")?
                    .state
                    .clone(),
            };
            #[cfg(feature = "libseccomp")]
            sync_seccomp(seccomp, &state, init_sender, main_receiver)
                .context("failed to sync seccomp with init")?;
        }
        if let Some(intel_rdt) = linux.intel_rdt() {
            let container_id = container_args
                .container
                .as_ref()
                .map(|container| container.id());
            need_to_clean_up_intel_rdt_subdirectory =
                setup_intel_rdt(container_id, &init_pid, intel_rdt)?;
        }
    }

    // We don't need to send anything to the init process after this point, so
    // close the sender.
    init_sender
        .close()
        .context("failed to close unused init sender")?;

    main_receiver
        .wait_for_init_ready()
        .context("failed to wait for init ready")?;

    tracing::debug!("init pid is {:?}", init_pid);

    // Before the main process returns, we want to make sure the intermediate
    // process is exit and reaped. By this point, the intermediate process
    // should already exited successfully. If intermediate process errors out,
    // the `init_ready` will not be sent.
    match waitpid(intermediate_pid, None) {
        Ok(WaitStatus::Exited(_, 0)) => (),
        Ok(WaitStatus::Exited(_, s)) => {
            tracing::warn!("intermediate process failed with exit status: {s}");
        }
        Ok(WaitStatus::Signaled(_, sig, _)) => {
            tracing::warn!("intermediate process killed with signal: {sig}")
        }
        Ok(_) => (),
        Err(nix::errno::Errno::ECHILD) => {
            // This is safe because intermediate_process and main_process check if the process is
            // finished by piping instead of exit code.
            tracing::warn!("intermediate process already reaped");
        }
        Err(err) => bail!("failed to wait for intermediate process: {err}"),
    };

    Ok((init_pid, need_to_clean_up_intel_rdt_subdirectory))
}

#[cfg(feature = "libseccomp")]
fn sync_seccomp(
    seccomp: &runtime::LinuxSeccomp,
    state: &ContainerProcessState,
    init_sender: &mut channel::InitSender,
    main_receiver: &mut channel::MainReceiver,
) -> Result<()> {
    if seccomp::is_notify(seccomp) {
        tracing::debug!("main process waiting for sync seccomp");
        let seccomp_fd = main_receiver.wait_for_seccomp_request()?;
        let listener_path = seccomp
            .listener_path()
            .as_ref()
            .context("notify will require seccomp listener path to be set")?;
        let encoded_state =
            serde_json::to_vec(state).context("failed to encode container process state")?;
        sync_seccomp_send_msg(listener_path, &encoded_state, seccomp_fd)
            .context("failed to send msg to seccomp listener")?;
        init_sender.seccomp_notify_done()?;
        // Once we sent the seccomp notify fd to the seccomp listener, we can
        // safely close the fd. The SCM_RIGHTS msg will duplicate the fd to the
        // process on the other end of the listener.
        let _ = unistd::close(seccomp_fd);
    }

    Ok(())
}

#[cfg(feature = "libseccomp")]
fn sync_seccomp_send_msg(listener_path: &Path, msg: &[u8], fd: i32) -> Result<()> {
    // The seccomp listener has specific instructions on how to transmit the
    // information through seccomp listener.  Therefore, we have to use
    // libc/nix APIs instead of Rust std lib APIs to maintain flexibility.
    let socket = socket::socket(
        socket::AddressFamily::Unix,
        socket::SockType::Stream,
        socket::SockFlag::empty(),
        None,
    )
    .context("failed to create unix domain socket for seccomp listener")?;
    let unix_addr = socket::UnixAddr::new(listener_path).context("failed to create unix addr")?;
    socket::connect(socket, &unix_addr).with_context(|| {
        format!("failed to connect to seccomp notify listerner path: {listener_path:?}")
    })?;
    // We have to use sendmsg here because the spec requires us to send seccomp notify fds through
    // SCM_RIGHTS message.
    // Ref: https://man7.org/linux/man-pages/man3/sendmsg.3p.html
    // Ref: https://man7.org/linux/man-pages/man3/cmsg.3.html
    let iov = [IoSlice::new(msg)];
    let fds = [fd];
    let cmsgs = socket::ControlMessage::ScmRights(&fds);
    socket::sendmsg::<UnixAddr>(socket, &iov, &[cmsgs], socket::MsgFlags::empty(), None)
        .context("failed to write container state to seccomp listener")?;
    // The spec requires the listener socket to be closed immediately after sending.
    let _ = unistd::close(socket);

    Ok(())
}

fn setup_mapping(rootless: &Rootless, pid: Pid) -> Result<()> {
    tracing::debug!("write mapping for pid {:?}", pid);
    if !rootless.privileged {
        // The main process is running as an unprivileged user and cannot write the mapping
        // until "deny" has been written to setgroups. See CVE-2014-8989.
        utils::write_file(format!("/proc/{pid}/setgroups"), "deny")?;
    }

    rootless
        .write_uid_mapping(pid)
        .context(format!("failed to map uid of pid {pid}"))?;
    rootless
        .write_gid_mapping(pid)
        .context(format!("failed to map gid of pid {pid}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::channel::{intermediate_channel, main_channel};
    use crate::rootless::RootlessIDMapper;
    use nix::{
        sched::{unshare, CloneFlags},
        unistd::{self, getgid, getuid},
    };
    use oci_spec::runtime::LinuxIdMappingBuilder;
    #[cfg(feature = "libseccomp")]
    use oci_spec::runtime::{LinuxSeccompAction, LinuxSeccompBuilder, LinuxSyscallBuilder};
    use serial_test::serial;
    use std::fs;

    #[test]
    #[serial]
    fn setup_uid_mapping_should_succeed() -> Result<()> {
        let uid_mapping = LinuxIdMappingBuilder::default()
            .host_id(getuid())
            .container_id(0u32)
            .size(1u32)
            .build()?;
        let uid_mappings = vec![uid_mapping];
        let tmp = tempfile::tempdir()?;
        let id_mapper = RootlessIDMapper::new_test(tmp.path().to_path_buf());
        let rootless = Rootless {
            uid_mappings: Some(&uid_mappings),
            privileged: true,
            rootless_id_mapper: id_mapper.clone(),
            ..Default::default()
        };
        let (mut parent_sender, mut parent_receiver) = main_channel()?;
        let (mut child_sender, mut child_receiver) = intermediate_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                parent_receiver.wait_for_mapping_request()?;
                parent_receiver.close()?;

                // In test, we fake the uid path in /proc/{pid}/uid_map, so we
                // need to ensure the path exists before we write the mapping.
                // The path requires the pid we use, so we can only do do after
                // obtaining the child pid here.
                id_mapper.ensure_uid_path(&child)?;
                setup_mapping(&rootless, child)?;
                let line = fs::read_to_string(id_mapper.get_uid_path(&child))?;
                let line_splited = line.split_whitespace();
                for (act, expect) in line_splited.zip([
                    uid_mapping.container_id().to_string(),
                    uid_mapping.host_id().to_string(),
                    uid_mapping.size().to_string(),
                ]) {
                    assert_eq!(act, expect);
                }
                child_sender.mapping_written()?;
                child_sender.close()?;
            }
            unistd::ForkResult::Child => {
                prctl::set_dumpable(true).unwrap();
                unshare(CloneFlags::CLONE_NEWUSER)?;
                parent_sender.identifier_mapping_request()?;
                parent_sender.close()?;
                child_receiver.wait_for_mapping_ack()?;
                child_receiver.close()?;
                std::process::exit(0);
            }
        }
        Ok(())
    }

    #[test]
    #[serial]
    fn setup_gid_mapping_should_successed() -> Result<()> {
        let gid_mapping = LinuxIdMappingBuilder::default()
            .host_id(getgid())
            .container_id(0u32)
            .size(1u32)
            .build()?;
        let gid_mappings = vec![gid_mapping];
        let tmp = tempfile::tempdir()?;
        let id_mapper = RootlessIDMapper::new_test(tmp.path().to_path_buf());
        let rootless = Rootless {
            gid_mappings: Some(&gid_mappings),
            rootless_id_mapper: id_mapper.clone(),
            ..Default::default()
        };
        let (mut parent_sender, mut parent_receiver) = main_channel()?;
        let (mut child_sender, mut child_receiver) = intermediate_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                parent_receiver.wait_for_mapping_request()?;
                parent_receiver.close()?;

                // In test, we fake the gid path in /proc/{pid}/gid_map, so we
                // need to ensure the path exists before we write the mapping.
                // The path requires the pid we use, so we can only do do after
                // obtaining the child pid here.
                id_mapper.ensure_gid_path(&child)?;
                setup_mapping(&rootless, child)?;
                let line = fs::read_to_string(id_mapper.get_gid_path(&child))?;
                let line_splited = line.split_whitespace();
                for (act, expect) in line_splited.zip([
                    gid_mapping.container_id().to_string(),
                    gid_mapping.host_id().to_string(),
                    gid_mapping.size().to_string(),
                ]) {
                    assert_eq!(act, expect);
                }
                assert_eq!(
                    fs::read_to_string(format!("/proc/{}/setgroups", child.as_raw()))?,
                    "deny\n",
                );
                child_sender.mapping_written()?;
                child_sender.close()?;
            }
            unistd::ForkResult::Child => {
                prctl::set_dumpable(true).unwrap();
                unshare(CloneFlags::CLONE_NEWUSER)?;
                parent_sender.identifier_mapping_request()?;
                parent_sender.close()?;
                child_receiver.wait_for_mapping_ack()?;
                child_receiver.close()?;
                std::process::exit(0);
            }
        }
        Ok(())
    }

    #[test]
    #[serial]
    #[cfg(feature = "libseccomp")]
    fn test_sync_seccomp() -> Result<()> {
        use std::io::Read;
        use std::os::unix::io::IntoRawFd;
        use std::os::unix::net::UnixListener;
        use std::thread;

        let tmp_dir = tempfile::tempdir()?;
        let scmp_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(tmp_dir.path().join("scmp_file"))?;

        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(tmp_dir.path().join("socket_file.sock"))?;

        let (mut main_sender, mut main_receiver) = channel::main_channel()?;
        let (mut init_sender, mut init_receiver) = channel::init_channel()?;
        let socket_path = tmp_dir.path().join("socket_file.sock");
        let socket_path_seccomp_th = socket_path.clone();

        let state = ContainerProcessState::default();
        let want = serde_json::to_string(&state)?;
        let th = thread::spawn(move || {
            sync_seccomp(
                &LinuxSeccompBuilder::default()
                    .listener_path(socket_path_seccomp_th)
                    .syscalls(vec![LinuxSyscallBuilder::default()
                        .action(LinuxSeccompAction::ScmpActNotify)
                        .build()
                        .unwrap()])
                    .build()
                    .unwrap(),
                &state,
                &mut init_sender,
                &mut main_receiver,
            )
            .unwrap();
        });

        let fd = scmp_file.into_raw_fd();
        assert!(main_sender.seccomp_notify_request(fd).is_ok());

        fs::remove_file(socket_path.clone())?;
        let lis = UnixListener::bind(socket_path)?;
        let (mut socket, _) = lis.accept()?;
        let mut got = String::new();
        socket.read_to_string(&mut got)?;
        assert!(init_receiver.wait_for_seccomp_request_done().is_ok());

        assert_eq!(want, got);
        assert!(th.join().is_ok());
        Ok(())
    }
}
