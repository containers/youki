use crate::{namespaces::Namespaces, process::channel, process::fork};
use libcgroups::common::CgroupManager;
use nix::unistd::{close, write};
use nix::unistd::{Gid, Pid, Uid};
use oci_spec::runtime::{LinuxNamespaceType, LinuxResources};
use procfs::process::Process;
use std::convert::From;

use super::args::{ContainerArgs, ContainerType};
use super::container_init_process::container_init_process;

#[derive(Debug, thiserror::Error)]
pub enum IntermediateProcessError {
    #[error("missing linux in spec")]
    NoLinuxSpec,
    #[error("missing process in spec")]
    NoProcessSpec,
    #[error(transparent)]
    Channel(#[from] channel::ChannelError),
    #[error(transparent)]
    Namespace(#[from] crate::namespaces::NamespaceError),
    #[error(transparent)]
    Syscall(#[from] crate::syscall::SyscallError),
    #[error("failed to launch init process")]
    InitProcess(#[source] fork::CloneError),
    #[error("cgroup error: {0}")]
    Cgroup(String),
    #[error(transparent)]
    Procfs(#[from] procfs::ProcError),
    #[error("exec notify failed")]
    ExecNotify(#[source] nix::Error),
}

type Result<T> = std::result::Result<T, IntermediateProcessError>;

pub fn container_intermediate_process(
    args: &ContainerArgs,
    intermediate_chan: &mut (channel::IntermediateSender, channel::IntermediateReceiver),
    init_chan: &mut (channel::InitSender, channel::InitReceiver),
    main_sender: &mut channel::MainSender,
) -> Result<Pid> {
    let (inter_sender, inter_receiver) = intermediate_chan;
    let (init_sender, init_receiver) = init_chan;
    let command = &args.syscall;
    let spec = &args.spec;
    let linux = spec
        .linux()
        .as_ref()
        .ok_or(IntermediateProcessError::NoLinuxSpec)?;
    let namespaces = Namespaces::from(linux.namespaces().as_ref());

    // this needs to be done before we create the init process, so that the init
    // process will already be captured by the cgroup. It also needs to be done
    // before we enter the user namespace because if a privileged user starts a
    // rootless container on a cgroup v1 system we can still fulfill resource
    // restrictions through the cgroup fs support (delegation through systemd is
    // not supported for v1 by us). This only works if the user has not yet been
    // mapped to an unprivileged user by the user namespace however.
    // In addition this needs to be done before we enter the cgroup namespace as
    // the cgroup of the process will form the root of the cgroup hierarchy in
    // the cgroup namespace.
    apply_cgroups(
        &args.cgroup_manager,
        linux.resources().as_ref(),
        matches!(args.container_type, ContainerType::InitContainer),
    )?;

    // if new user is specified in specification, this will be true and new
    // namespace will be created, check
    // https://man7.org/linux/man-pages/man7/user_namespaces.7.html for more
    // information
    if let Some(user_namespace) = namespaces.get(LinuxNamespaceType::User) {
        namespaces.unshare_or_setns(user_namespace)?;
        if user_namespace.path().is_none() {
            tracing::debug!("creating new user namespace");
            // child needs to be dumpable, otherwise the non root parent is not
            // allowed to write the uid/gid maps
            prctl::set_dumpable(true).unwrap();
            main_sender.identifier_mapping_request().map_err(|err| {
                tracing::error!("failed to send id mapping request: {}", err);
                err
            })?;
            inter_receiver.wait_for_mapping_ack().map_err(|err| {
                tracing::error!("failed to receive id mapping ack: {}", err);
                err
            })?;
            prctl::set_dumpable(false).unwrap();
        }

        // After UID and GID mapping is configured correctly in the Youki main
        // process, We want to make sure continue as the root user inside the
        // new user namespace. This is required because the process of
        // configuring the container process will require root, even though the
        // root in the user namespace likely is mapped to an non-privileged user
        // on the parent user namespace.
        command.set_id(Uid::from_raw(0), Gid::from_raw(0))?;
    }

    // set limits and namespaces to the process
    let proc = spec
        .process()
        .as_ref()
        .ok_or(IntermediateProcessError::NoProcessSpec)?;
    if let Some(rlimits) = proc.rlimits() {
        for rlimit in rlimits {
            command.set_rlimit(rlimit)?;
        }
    }

    // Pid namespace requires an extra fork to enter, so we enter pid namespace now.
    if let Some(pid_namespace) = namespaces.get(LinuxNamespaceType::Pid) {
        namespaces.unshare_or_setns(pid_namespace)?;
    }

    // We have to record the pid of the init process. The init process will be
    // inside the pid namespace, so we can't rely on the init process to send us
    // the correct pid. We also want to clone the init process as a sibling
    // process to the intermediate process. The intermediate process is only
    // used as a jumping board to set the init process to the correct
    // configuration. The youki main process can decide what to do with the init
    // process and the intermediate process can just exit safely after the job
    // is done.
    let pid = fork::container_clone_sibling("youki:[2:INIT]", || {
        // We are inside the forked process here. The first thing we have to do
        // is to close any unused senders, since fork will make a dup for all
        // the socket.
        init_sender.close().map_err(|err| {
            tracing::error!("failed to close receiver in init process: {}", err);
            IntermediateProcessError::Channel(err)
        })?;
        inter_sender.close().map_err(|err| {
            tracing::error!(
                "failed to close sender in the intermediate process: {}",
                err
            );
            IntermediateProcessError::Channel(err)
        })?;
        match container_init_process(args, main_sender, init_receiver) {
            Ok(_) => Ok(0),
            Err(e) => {
                if let ContainerType::TenantContainer { exec_notify_fd } = args.container_type {
                    let buf = format!("{e}");
                    write(exec_notify_fd, buf.as_bytes()).map_err(|err| {
                        tracing::error!("failed to write to exec notify fd: {}", err);
                        IntermediateProcessError::ExecNotify(err)
                    })?;
                    close(exec_notify_fd).map_err(|err| {
                        tracing::error!("failed to close exec notify fd: {}", err);
                        IntermediateProcessError::ExecNotify(err)
                    })?;
                }
                tracing::error!("failed to initialize container process: {e}");
                Err(e.into())
            }
        }
    })
    .map_err(|err| {
        tracing::error!("failed to fork init process: {}", err);
        IntermediateProcessError::InitProcess(err)
    })?;

    // Close the exec_notify_fd in this process
    if let ContainerType::TenantContainer { exec_notify_fd } = args.container_type {
        close(exec_notify_fd).map_err(|err| {
            tracing::error!("failed to close exec notify fd: {}", err);
            IntermediateProcessError::ExecNotify(err)
        })?;
    }

    main_sender.intermediate_ready(pid).map_err(|err| {
        tracing::error!("failed to wait on intermediate process: {}", err);
        err
    })?;

    // Close unused senders here so we don't have lingering socket around.
    main_sender.close().map_err(|err| {
        tracing::error!("failed to close unused main sender: {}", err);
        err
    })?;
    inter_sender.close().map_err(|err| {
        tracing::error!(
            "failed to close sender in the intermediate process: {}",
            err
        );
        err
    })?;
    init_sender.close().map_err(|err| {
        tracing::error!("failed to close unused init sender: {}", err);
        err
    })?;
    Ok(pid)
}

fn apply_cgroups<
    C: CgroupManager<Error = E> + ?Sized,
    E: std::error::Error + Send + Sync + 'static,
>(
    cmanager: &C,
    resources: Option<&LinuxResources>,
    init: bool,
) -> Result<()> {
    let pid = Pid::from_raw(Process::myself()?.pid());
    cmanager.add_task(pid).map_err(|err| {
        tracing::error!(?pid, ?err, ?init, "failed to add task to cgroup");
        IntermediateProcessError::Cgroup(err.to_string())
    })?;

    if let Some(resources) = resources {
        if init {
            let controller_opt = libcgroups::common::ControllerOpt {
                resources,
                freezer_state: None,
                oom_score_adj: None,
                disable_oom_killer: false,
            };

            cmanager.apply(&controller_opt).map_err(|err| {
                tracing::error!(?pid, ?err, ?init, "failed to apply cgroup");
                IntermediateProcessError::Cgroup(err.to_string())
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_cgroups;
    use anyhow::Result;
    use libcgroups::test_manager::TestManager;
    use nix::unistd::Pid;
    use oci_spec::runtime::LinuxResources;
    use procfs::process::Process;

    #[test]
    fn apply_cgroup_init() -> Result<()> {
        // arrange
        let cmanager = TestManager::default();
        let resources = LinuxResources::default();

        // act
        apply_cgroups(&cmanager, Some(&resources), true)?;

        // assert
        assert!(cmanager.get_add_task_args().len() == 1);
        assert_eq!(
            cmanager.get_add_task_args()[0],
            Pid::from_raw(Process::myself()?.pid())
        );
        assert!(cmanager.apply_called());
        Ok(())
    }

    #[test]
    fn apply_cgroup_tenant() -> Result<()> {
        // arrange
        let cmanager = TestManager::default();
        let resources = LinuxResources::default();

        // act
        apply_cgroups(&cmanager, Some(&resources), false)?;

        // assert
        assert_eq!(
            cmanager.get_add_task_args()[0],
            Pid::from_raw(Process::myself()?.pid())
        );
        assert!(!cmanager.apply_called());
        Ok(())
    }

    #[test]
    fn apply_cgroup_no_resources() -> Result<()> {
        // arrange
        let cmanager = TestManager::default();

        // act
        apply_cgroups(&cmanager, None, true)?;
        // assert
        assert_eq!(
            cmanager.get_add_task_args()[0],
            Pid::from_raw(Process::myself()?.pid())
        );
        assert!(!cmanager.apply_called());
        Ok(())
    }
}
