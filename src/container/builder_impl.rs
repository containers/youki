use anyhow::{Context, Result};
use oci_spec::Spec;
use std::{fs, path::PathBuf};

use crate::{
    cgroups,
    namespaces::Namespaces,
    process::{child, fork, init, parent},
    rootless::Rootless,
    stdio::FileDescriptor,
    syscall::linux::LinuxSyscall,
    utils,
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
    /// File descriptos preserved/passed to the container init process.
    pub preserve_fds: i32,
}

impl ContainerBuilderImpl {
    pub(super) fn create(&mut self) -> Result<()> {
        self.run_container()?;

        Ok(())
    }

    fn run_container(&mut self) -> Result<()> {
        prctl::set_dumpable(false).unwrap();

        let linux = self.spec.linux.as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;
        let namespaces: Namespaces = linux.namespaces.clone().into();

        // create the parent and child process structure so the parent and child process can sync with each other
        let (mut parent, parent_channel) = parent::ParentProcess::new(self.rootless.clone())?;
        let child = child::ChildProcess::new(parent_channel)?;

        // This init_args will be passed to the container init process,
        // therefore we will have to move all the variable by value. Since self
        // is a shared reference, we have to clone these variables here.
        let init_args = init::ContainerInitArgs {
            init: self.init,
            syscall: self.syscall.clone(),
            spec: self.spec.clone(),
            rootfs: self.rootfs.clone(),
            console_socket: self.console_socket.clone(),
            rootless: self.rootless.clone(),
            notify_path: self.notify_path.clone(),
            preserve_fds: self.preserve_fds,
            child,
        };

        // We have to box up this closure to correctly pass to the init function
        // of the new process.
        let cb = Box::new(move || {
            if let Err(error) = init::container_init(init_args) {
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
            cmanager.apply(linux.resources.as_ref().unwrap())?;
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
