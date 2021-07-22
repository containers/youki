use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use nix::{
    sched,
    unistd::{Gid, Uid},
};
use oci_spec::Spec;

use crate::{
    capabilities, cgroups,
    command::{linux::LinuxSyscall, Syscall},
    namespaces::Namespaces,
    notify_socket::NotifyListener,
    process::{child, fork, parent},
    rootfs,
    rootless::Rootless,
    stdio::FileDescriptor,
    tty, utils,
};

use super::{Container, ContainerStatus};

pub(super) struct ContainerBuilderImpl {
    /// Flag indicating if an init or a tenant container should be created
    pub init: bool,
    /// Interface to operating system primitives
    pub syscall: LinuxSyscall,
    /// Flag indicating if systemd should be used for cgroup management
    pub use_systemd: bool,
    /// Id of the container
    pub container_id: String,
    /// Directory where the state of the container will be stored
    pub container_dir: PathBuf,
    /// OCI complient runtime spec
    pub spec: Spec,
    /// Root filesystem of the container
    pub rootfs: PathBuf,
    /// File which will be used to communicate the pid of the
    /// container process to the higher level runtime
    pub pid_file: Option<PathBuf>,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<FileDescriptor>,
    /// Options for rootless containers
    pub rootless: Option<Rootless>,
    /// Path to the Unix Domain Socket to communicate container start
    pub notify_path: PathBuf,
    /// Container state
    pub container: Option<Container>,
}

impl ContainerBuilderImpl {
    pub(super) fn create(&mut self) -> Result<()> {
        self.run_container()?;

        Ok(())
    }

    fn run_container(&mut self) -> Result<()> {
        prctl::set_dumpable(false).unwrap();

        let linux = self.spec.linux.as_ref().unwrap();
        let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;
        let namespaces: Namespaces = linux.namespaces.clone().into();

        // create the parent and child process structure so the parent and child process can sync with each other
        let (mut parent, parent_channel) = parent::ParentProcess::new(self.rootless.clone())?;
        let mut child = child::ChildProcess::new(parent_channel)?;

        let cb = Box::new(|| {
            if let Err(error) = container_init(
                self.init,
                self.rootless.clone(),
                self.spec.clone(),
                self.syscall.clone(),
                self.rootfs.clone(),
                self.console_socket.clone(),
                self.notify_path.clone(),
                &mut child,
            ) {
                log::debug!("failed to run container_init: {:?}", error);
                return -1;
            }

            0
        });

        let init_pid = fork::clone(cb, namespaces.clone_flags)?;
        log::debug!("init pid is {:?}", init_pid);

        parent.wait_for_child_ready(init_pid)?;

        cmanager.add_task(init_pid)?;
        if self.rootless.is_none() && linux.resources.is_some() && self.init {
            cmanager.apply(&linux.resources.as_ref().unwrap())?;
        }

        // if file to write the pid to is specified, write pid of the child
        if let Some(pid_file) = &self.pid_file {
            fs::write(&pid_file, format!("{}", init_pid))?;
        }

        if let Some(container) = &self.container {
            // update status and pid of the container process
            container
                .update_status(ContainerStatus::Created)
                .set_creator(nix::unistd::geteuid().as_raw())
                .set_pid(init_pid.as_raw())
                .save()?;
        }

        Ok(())
    }
}

fn container_init(
    init: bool,
    rootless: Option<Rootless>,
    spec: Spec,
    command: LinuxSyscall,
    rootfs: PathBuf,
    console_socket: Option<FileDescriptor>,
    notify_name: PathBuf,
    child: &mut child::ChildProcess,
) -> Result<()> {
    let linux = spec.linux.as_ref().unwrap();
    let namespaces: Namespaces = linux.namespaces.clone().into();
    // need to create the notify socket before we pivot root, since the unix
    // domain socket used here is outside of the rootfs of container
    let mut notify_socket: NotifyListener = NotifyListener::new(&notify_name)?;
    let proc = &spec.process;

    // if Out-of-memory score adjustment is set in specification.  set the score
    // value for the current process check
    // https://dev.to/rrampage/surviving-the-linux-oom-killer-2ki9 for some more
    // information
    if let Some(ref resource) = linux.resources {
        if let Some(oom_score_adj) = resource.oom_score_adj {
            let mut f = fs::File::create("/proc/self/oom_score_adj")?;
            f.write_all(oom_score_adj.to_string().as_bytes())?;
        }
    }

    // if new user is specified in specification, this will be true and new
    // namespace will be created, check
    // https://man7.org/linux/man-pages/man7/user_namespaces.7.html for more
    // information
    if rootless.is_some() {
        // child needs to be dumpable, otherwise the non root parent is not
        // allowed to write the uid/gid maps
        prctl::set_dumpable(true).unwrap();
        child.request_identifier_mapping()?;
        child.wait_for_mapping_ack()?;
        prctl::set_dumpable(false).unwrap();
    }

    // set limits and namespaces to the process
    for rlimit in spec.process.rlimits.iter() {
        command.set_rlimit(rlimit).context("failed to set rlimit")?;
    }

    command
        .set_id(Uid::from_raw(0), Gid::from_raw(0))
        .context("failed to become root")?;

    // set up tty if specified
    if let Some(csocketfd) = console_socket {
        tty::setup_console(&csocketfd)?;
    }

    // join existing namespaces
    namespaces.apply_setns()?;

    command.set_hostname(spec.hostname.as_str())?;

    if proc.no_new_privileges {
        let _ = prctl::set_no_new_privileges(true);
    }

    if init {
        rootfs::prepare_rootfs(
            &spec,
            &rootfs,
            namespaces
                .clone_flags
                .contains(sched::CloneFlags::CLONE_NEWUSER),
        )
        .with_context(|| "Failed to prepare rootfs")?;

        // change the root of filesystem of the process to the rootfs
        command
            .pivot_rootfs(&rootfs)
            .with_context(|| format!("Failed to pivot root to {:?}", &rootfs))?;
    }

    command.set_id(Uid::from_raw(proc.user.uid), Gid::from_raw(proc.user.gid))?;
    capabilities::reset_effective(&command)?;
    if let Some(caps) = &proc.capabilities {
        capabilities::drop_privileges(&caps, &command)?;
    }

    // notify parents that the init process is ready to execute the payload.
    child.notify_parent()?;

    // listing on the notify socket for container start command
    notify_socket.wait_for_container_start()?;

    let args: &Vec<String> = &spec.process.args;
    let envs: &Vec<String> = &spec.process.env;
    utils::do_exec(&args[0], args, envs)?;

    // After do_exec is called, the process is replaced with the container
    // payload through execvp, so it should never reach here.
    unreachable!();
}
