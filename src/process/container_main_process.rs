use crate::{
    process::{args::ContainerArgs, channel, container_intermediate_process, fork},
    rootless::Rootless,
    utils,
};
use anyhow::{Context, Result};
use nix::unistd::Pid;

pub fn container_main_process(container_args: &ContainerArgs) -> Result<Pid> {
    // We use a set of channels to communicate between parent and child process. Each channel is uni-directional.
    let (main_sender, main_receiver) = &mut channel::main_channel()?;
    let (intermediate_sender, intermediate_receiver) = &mut channel::intermediate_channel()?;

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

    intermediate_sender
        .close()
        .context("failed to close unused sender")?;

    let init_pid = main_receiver.wait_for_intermediate_ready()?;
    log::debug!("init pid is {:?}", init_pid);

    Ok(init_pid)
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
