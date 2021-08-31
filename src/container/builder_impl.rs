use crate::{
    hooks,
    notify_socket::NotifyListener,
    process::{channel, fork, init},
    rootless::{self, Rootless},
    syscall::linux::LinuxSyscall,
    utils,
};
use anyhow::{Context, Result};
use cgroups;
use oci_spec::Spec;
use std::{fs, io::Write, os::unix::prelude::RawFd, path::PathBuf};

use super::{Container, ContainerStatus};

pub(super) struct ContainerBuilderImpl<'a> {
    /// Flag indicating if an init or a tenant container should be created
    pub init: bool,
    /// Interface to operating system primitives
    pub syscall: LinuxSyscall,
    /// Flag indicating if systemd should be used for cgroup management
    pub use_systemd: bool,
    /// Id of the container
    pub container_id: String,
    /// OCI complient runtime spec
    pub spec: &'a Spec,
    /// Root filesystem of the container
    pub rootfs: PathBuf,
    /// File which will be used to communicate the pid of the
    /// container process to the higher level runtime
    pub pid_file: Option<PathBuf>,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// Options for rootless containers
    pub rootless: Option<Rootless<'a>>,
    /// Path to the Unix Domain Socket to communicate container start
    pub notify_path: PathBuf,
    /// Container state
    pub container: Option<Container>,
    /// File descriptos preserved/passed to the container init process.
    pub preserve_fds: i32,
}

impl<'a> ContainerBuilderImpl<'a> {
    pub(super) fn create(&mut self) -> Result<()> {
        self.run_container()?;

        Ok(())
    }

    fn run_container(&mut self) -> Result<()> {
        let linux = self.spec.linux.as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;
        let process = self.spec.process.as_ref().context("No process in spec")?;

        if self.init {
            if let Some(hooks) = self.spec.hooks.as_ref() {
                hooks::run_hooks(hooks.create_runtime.as_ref(), self.container.as_ref())?
            }
        }

        // We use a set of channels to communicate between parent and child process. Each channel is uni-directional.
        let (sender_to_intermediate, receiver_from_main) = &mut channel::main_to_intermediate()?;
        let (sender_to_main, receiver_from_intermediate) = &mut channel::intermediate_to_main()?;

        // Need to create the notify socket before we pivot root, since the unix
        // domain socket used here is outside of the rootfs of container. During
        // exec, need to create the socket before we exter into existing mount
        // namespace.
        let notify_socket: NotifyListener = NotifyListener::new(&self.notify_path)?;

        // If Out-of-memory score adjustment is set in specification.  set the score
        // value for the current process check
        // https://dev.to/rrampage/surviving-the-linux-oom-killer-2ki9 for some more
        // information.
        //
        // This has to be done before !dumpable because /proc/self/oom_score_adj
        // is not writeable unless you're an privileged user (if !dumpable is
        // set). All children inherit their parent's oom_score_adj value on
        // fork(2) so this will always be propagated properly.
        if let Some(oom_score_adj) = process.oom_score_adj {
            log::debug!("Set OOM score to {}", oom_score_adj);
            let mut f = fs::File::create("/proc/self/oom_score_adj")?;
            f.write_all(oom_score_adj.to_string().as_bytes())?;
        }

        // Make the process non-dumpable, to avoid various race conditions that
        // could cause processes in namespaces we're joining to access host
        // resources (or potentially execute code).
        //
        // However, if the number of namespaces we are joining is 0, we are not
        // going to be switching to a different security context. Thus setting
        // ourselves to be non-dumpable only breaks things (like rootless
        // containers), which is the recommendation from the kernel folks.
        if linux.namespaces.is_some() {
            prctl::set_dumpable(false).unwrap();
        }

        // This init_args will be passed to the container init process,
        // therefore we will have to move all the variable by value. Since self
        // is a shared reference, we have to clone these variables here.
        let init_args = init::ContainerInitArgs {
            init: self.init,
            syscall: self.syscall.clone(),
            spec: self.spec.clone(),
            rootfs: self.rootfs.clone(),
            console_socket: self.console_socket,
            notify_socket,
            preserve_fds: self.preserve_fds,
            container: self.container.clone(),
        };
        let intermediate_pid = fork::container_fork(|| {
            // The fds in the pipe is duplicated during fork, so we first close
            // the unused fds. Note, this already runs in the child process.
            sender_to_intermediate
                .close()
                .context("Failed to close unused sender")?;
            receiver_from_intermediate
                .close()
                .context("Failed to close unused receiver")?;

            init::container_intermidiate(init_args, receiver_from_main, sender_to_main)
        })?;
        // Close down unused fds. The corresponding fds are duplicated to the
        // child process during fork.
        receiver_from_main
            .close()
            .context("Failed to close parent to child receiver")?;
        sender_to_main
            .close()
            .context("Failed to close child to parent sender")?;

        // If creating a rootless container, the intermediate process will ask
        // the main process to set up uid and gid mapping, once the intermediate
        // process enters into a new user namespace.
        if self.rootless.is_some() {
            receiver_from_intermediate.wait_for_mapping_request()?;
            log::debug!("write mapping for pid {:?}", intermediate_pid);
            utils::write_file(format!("/proc/{}/setgroups", intermediate_pid), "deny")?;
            rootless::write_uid_mapping(intermediate_pid, self.rootless.as_ref())?;
            rootless::write_gid_mapping(intermediate_pid, self.rootless.as_ref())?;
            sender_to_intermediate.mapping_written()?;
        }

        let init_pid = receiver_from_intermediate.wait_for_intermediate_ready()?;
        log::debug!("init pid is {:?}", init_pid);

        cmanager
            .add_task(init_pid)
            .context("Failed to add tasks to cgroup manager")?;
        if self.rootless.is_none() && linux.resources.is_some() && self.init {
            cmanager
                .apply(linux.resources.as_ref().unwrap())
                .context("Failed to apply resource limits through cgroup")?;
        }

        // if file to write the pid to is specified, write pid of the child
        if let Some(pid_file) = &self.pid_file {
            fs::write(&pid_file, format!("{}", init_pid)).context("Failed to write pid file")?;
        }

        if let Some(container) = &self.container {
            // update status and pid of the container process
            container
                .update_status(ContainerStatus::Created)
                .set_creator(nix::unistd::geteuid().as_raw())
                .set_pid(init_pid.as_raw())
                .save()
                .context("Failed to save container state")?;
        }

        Ok(())
    }
}
