use crate::{
    container::ContainerProcessState,
    process::{args::ContainerArgs, channel, container_intermediate_process, fork},
    rootless::Rootless,
    seccomp, utils,
};
use anyhow::{Context, Result};
use nix::{
    sys::{socket, uio},
    unistd::{self, Pid},
};
use oci_spec::runtime;
use std::path::Path;

pub fn container_main_process(container_args: &ContainerArgs) -> Result<Pid> {
    // We use a set of channels to communicate between parent and child process. Each channel is uni-directional.
    let (main_sender, main_receiver) = &mut channel::main_channel()?;
    let (intermediate_sender, intermediate_receiver) = &mut channel::intermediate_channel()?;
    let (init_sender, init_receiver) = &mut channel::init_channel()?;

    let intermediate_pid = fork::container_fork(|| {
        // The fds in the channel is duplicated during fork, so we first close
        // the unused fds. Note, this already runs in the child process.
        main_receiver
            .close()
            .context("failed to close unused receiver")?;

        container_intermediate_process::container_intermediate_process(
            container_args,
            intermediate_sender,
            intermediate_receiver,
            init_sender,
            init_receiver,
            main_sender,
        )
    })?;
    // Close down unused fds. The corresponding fds are duplicated to the
    // child process during fork.
    main_sender
        .close()
        .context("failed to close unused sender")?;

    // If creating a rootless container, the intermediate process will ask
    // the main process to set up uid and gid mapping, once the intermediate
    // process enters into a new user namespace.
    if let Some(rootless) = &container_args.rootless {
        main_receiver.wait_for_mapping_request()?;
        setup_mapping(rootless, intermediate_pid)?;
        intermediate_sender.mapping_written()?;
    }

    // The intermediate process will send the init pid once it forks the init
    // process.  The intermediate process should exit after this point.
    let init_pid = main_receiver.wait_for_intermediate_ready()?;

    intermediate_sender
        .close()
        .context("failed to close unused sender")?;

    if let Some(linux) = container_args.spec.linux() {
        if let Some(seccomp) = linux.seccomp() {
            let seccomp_metadata = if let Some(metadata) = seccomp.listener_metadata() {
                metadata.to_owned()
            } else {
                String::new()
            };
            let state = ContainerProcessState {
                oci_version: container_args.spec.version().to_string(),
                // runc hardcode the `seccompFd` name for fds.
                fds: vec![String::from("seccompFd")],
                pid: init_pid.as_raw(),
                metadata: seccomp_metadata,
                state: container_args
                    .container
                    .as_ref()
                    .context("container state is required")?
                    .state
                    .clone(),
            };
            sync_seccomp(seccomp, &state, init_sender, main_receiver)
                .context("failed to sync seccomp with init")?;
        }
    }

    init_sender
        .close()
        .context("failed to close unused init sender")?;

    main_receiver
        .wait_for_init_ready()
        .context("failed to wait for init ready")?;

    log::debug!("init pid is {:?}", init_pid);

    Ok(init_pid)
}

fn sync_seccomp(
    seccomp: &runtime::LinuxSeccomp,
    state: &ContainerProcessState,
    init_sender: &mut channel::InitSender,
    main_receiver: &mut channel::MainReceiver,
) -> Result<()> {
    if seccomp::is_notify(seccomp) {
        log::debug!("main process waiting for sync seccomp");
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
    }

    Ok(())
}

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
    let unix_addr =
        socket::SockAddr::new_unix(listener_path).context("failed to create unix addr")?;
    socket::connect(socket, &unix_addr).with_context(|| {
        format!(
            "failed to connect to seccomp notify listerner path: {:?}",
            listener_path
        )
    })?;
    // We have to use sendmsg here because the spec requires us to send seccomp notify fds through
    // SCM_RIGHTS message.
    // Ref: https://man7.org/linux/man-pages/man3/sendmsg.3p.html
    // Ref: https://man7.org/linux/man-pages/man3/cmsg.3.html
    let iov = [uio::IoVec::from_slice(msg)];
    let fds = [fd];
    let cmsgs = socket::ControlMessage::ScmRights(&fds);
    socket::sendmsg(socket, &iov, &[cmsgs], socket::MsgFlags::empty(), None)
        .context("failed to write container state to seccomp listener")?;
    let _ = unistd::close(socket);

    Ok(())
}

fn setup_mapping(rootless: &Rootless, pid: Pid) -> Result<()> {
    log::debug!("write mapping for pid {:?}", pid);
    if !rootless.privileged {
        // The main process is running as an unprivileged user and cannot write the mapping
        // until "deny" has been written to setgroups. See CVE-2014-8989.
        utils::write_file(format!("/proc/{}/setgroups", pid), "deny")?;
    }

    rootless
        .write_uid_mapping(pid)
        .context(format!("failed to map uid of pid {}", pid))?;
    rootless
        .write_gid_mapping(pid)
        .context(format!("failed to map gid of pid {}", pid))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::channel::{intermediate_channel, main_channel};
    use nix::{
        sched::{unshare, CloneFlags},
        unistd::{self, getgid, getuid},
    };
    use oci_spec::runtime::LinuxIdMappingBuilder;
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
        let rootless = Rootless {
            uid_mappings: Some(&uid_mappings),
            privileged: true,
            ..Default::default()
        };
        let (mut parent_sender, mut parent_receiver) = main_channel()?;
        let (mut child_sender, mut child_receiver) = intermediate_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                parent_receiver.wait_for_mapping_request()?;
                parent_receiver.close()?;
                setup_mapping(&rootless, child)?;
                let line = fs::read_to_string(format!("/proc/{}/uid_map", child.as_raw()))?;
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
        let rootless = Rootless {
            gid_mappings: Some(&gid_mappings),
            ..Default::default()
        };
        let (mut parent_sender, mut parent_receiver) = main_channel()?;
        let (mut child_sender, mut child_receiver) = intermediate_channel()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                parent_receiver.wait_for_mapping_request()?;
                parent_receiver.close()?;
                setup_mapping(&rootless, child)?;
                let line = fs::read_to_string(format!("/proc/{}/gid_map", child.as_raw()))?;
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
}
