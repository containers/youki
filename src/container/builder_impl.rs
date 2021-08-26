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
use std::{fs, os::unix::prelude::RawFd, path::PathBuf};

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
        prctl::set_dumpable(false).unwrap();

        let linux = self.spec.linux.as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;

        if self.init {
            if let Some(hooks) = self.spec.hooks.as_ref() {
                hooks::run_hooks(hooks.create_runtime.as_ref(), self.container.as_ref())?
            }
        }

        // We use a set of channels to communicate between parent and child process. Each channel is uni-directional.
        let parent_to_child = &mut channel::Channel::new()?;
        let child_to_parent = &mut channel::Channel::new()?;

        // Need to create the notify socket before we pivot root, since the unix
        // domain socket used here is outside of the rootfs of container. During
        // exec, need to create the socket before we exter into existing mount
        // namespace.
        let notify_socket: NotifyListener = NotifyListener::new(&self.notify_path)?;

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
            init::container_intermidiate(init_args, parent_to_child, child_to_parent)
        })?;
        // If creating a rootless container, the intermediate process will ask
        // the main process to set up uid and gid mapping, once the intermediate
        // process enters into a new user namespace.
        if self.rootless.is_some() {
            child_to_parent.wait_for_mapping_request()?;
            log::debug!("write mapping for pid {:?}", intermediate_pid);
            utils::write_file(format!("/proc/{}/setgroups", intermediate_pid), "deny")?;
            rootless::write_uid_mapping(intermediate_pid, self.rootless.as_ref())?;
            rootless::write_gid_mapping(intermediate_pid, self.rootless.as_ref())?;
            parent_to_child.send_mapping_written()?;
        }

        let init_pid = child_to_parent.wait_for_child_ready()?;
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
