use std::path::PathBuf;

use anyhow::Result;
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
    pub init: bool,
    pub syscall: LinuxSyscall,
    pub use_systemd: bool,
    pub container_id: String,
    pub root_path: PathBuf,
    pub container_dir: PathBuf,
    pub spec: Spec,
    pub rootfs: PathBuf,
    pub pid_file: Option<PathBuf>,
    pub console_socket: Option<FileDescriptor>,
    pub rootless: Option<Rootless>,
    pub notify_socket: NotifyListener,
    pub container: Option<Container>,
}

impl ContainerBuilderImpl {
    pub(super) fn create(&mut self) -> Result<()> {
        if let Process::Parent(_) = self.run_container()? {
            std::process::exit(0);
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
                    self.syscall.set_rlimit(rlimit)?
                }
                self.syscall.set_id(Uid::from_raw(0), Gid::from_raw(0))?;

                let without = sched::CloneFlags::CLONE_NEWUSER;
                namespaces.apply_unshare(without)?;

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
                        setup_init_process(
                            &self.spec,
                            &self.syscall,
                            self.rootfs.clone(),
                            &namespaces,
                        )?;
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
