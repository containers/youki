use std::path::PathBuf;

use anyhow::{Context, Result};
use nix::{
    sched,
    unistd::{Gid, Uid},
};
use oci_spec::Spec;

use crate::{
    cgroups,
    command::{linux::LinuxSyscall, Syscall},
    namespaces::Namespaces,
    notify_socket::NotifyListener,
    process::{fork, setup_init_process, Process},
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
    /// Socket to communicate container start
    pub notify_socket: NotifyListener,
    /// Container state
    pub container: Option<Container>,
}

impl ContainerBuilderImpl {
    pub(super) fn create(&mut self) -> Result<()> {
        if let Process::Parent(_) = self.run_container()? {
            if self.init {
                std::process::exit(0);
            }
        }

        Ok(())
    }

    fn run_container(&mut self) -> Result<Process> {
        prctl::set_dumpable(false).unwrap();

        let linux = self.spec.linux.as_ref().unwrap();
        let namespaces: Namespaces = linux.namespaces.clone().into();

        let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;

        // first fork, which creates process, which will later create actual container process
        match fork::fork_first(
            self.init,
            &self.pid_file,
            &self.rootless,
            linux,
            self.container.as_ref(),
            cmanager,
        )? {
            // In the parent process, which called run_container
            Process::Parent(parent) => Ok(Process::Parent(parent)),
            // in child process
            Process::Child(child) => {
                // set limits and namespaces to the process
                for rlimit in self.spec.process.rlimits.iter() {
                    self.syscall
                        .set_rlimit(rlimit)
                        .context("failed to set rlimit")?;
                }
                self.syscall
                    .set_id(Uid::from_raw(0), Gid::from_raw(0))
                    .context("failed to become root")?;

                let without = sched::CloneFlags::CLONE_NEWUSER;
                namespaces
                    .apply_unshare(without)
                    .context("could not unshare namespaces")?;

                // set up tty if specified
                if let Some(csocketfd) = &self.console_socket {
                    tty::setup_console(csocketfd)?;
                }

                // set namespaces
                namespaces.apply_setns()?;

                // fork second time, which will later create container
                match fork::fork_init(child)? {
                    Process::Child(_child) => unreachable!(),
                    // This is actually the child process after fork
                    Process::Init(mut init) => {
                        // prepare process
                        if self.init {
                            setup_init_process(
                                &self.spec,
                                &self.syscall,
                                self.rootfs.clone(),
                                &namespaces,
                            )?;
                        }

                        init.ready()?;
                        self.notify_socket.wait_for_container_start()?;
                        // actually run the command / program to be run in container
                        let args: &Vec<String> = &self.spec.process.args;
                        let envs: &Vec<String> = &self.spec.process.env;
                        utils::do_exec(&args[0], args, envs)?;

                        if let Some(container) = &self.container {
                            // the command / program is done executing
                            container
                                .refresh_state()?
                                .update_status(ContainerStatus::Stopped)
                                .save()?;
                        }

                        Ok(Process::Init(init))
                    }
                    Process::Parent(_) => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }
}
