use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Result};
use clap::Clap;
use nix::sched;
use nix::unistd;
use nix::unistd::{Gid, Uid};

use crate::cgroups;
use crate::container::{Container, ContainerStatus};
use crate::namespaces::Namespaces;
use crate::notify_socket::NotifyListener;
use crate::process::{fork, Process};
use crate::rootfs;
use crate::spec;
use crate::stdio::FileDescriptor;
use crate::tty;
use crate::utils;
use crate::{capabilities, command::Command};

#[derive(Clap, Debug)]
pub struct Create {
    #[clap(short, long)]
    pid_file: Option<String>,
    #[clap(short, long, default_value = ".")]
    bundle: PathBuf,
    #[clap(short, long)]
    console_socket: Option<String>,
    pub container_id: String,
}

impl Create {
    pub fn exec(&self, root_path: PathBuf, command: impl Command) -> Result<()> {
        let container_dir = root_path.join(&self.container_id);
        if !container_dir.exists() {
            fs::create_dir(&container_dir).unwrap();
        } else {
            bail!("{} already exists", self.container_id)
        }

        unistd::chdir(&self.bundle)?;

        let spec = spec::Spec::load("config.json")?;
        fs::copy("config.json", container_dir.join("config.json"))?;
        log::debug!("spec: {:?}", spec);

        let container_dir = fs::canonicalize(container_dir)?;
        unistd::chdir(&*container_dir)?;

        log::debug!("{:?}", &container_dir);
        let container = Container::new(
            &self.container_id,
            ContainerStatus::Creating,
            None,
            self.bundle.to_str().unwrap(),
            &container_dir,
        )?;
        container.save()?;

        let mut notify_socket: NotifyListener = NotifyListener::new(&container_dir)?;

        let rootfs = fs::canonicalize(&spec.root.path)?;

        let (csocketfd, _consolefd) = {
            if let Some(console_socket) = &self.console_socket {
                let (csocketfd, consolefd) =
                    tty::load_console_sockets(&container_dir, console_socket)?;
                (Some(csocketfd), Some(consolefd))
            } else {
                (None, None)
            }
        };

        let process = run_container(
            self.pid_file.as_ref(),
            &mut notify_socket,
            rootfs,
            spec,
            csocketfd,
            container,
            command,
        )?;
        if let Process::Parent(_) = process {
            process::exit(0);
        }
        Ok(())
    }
}

fn run_container<P: AsRef<Path>>(
    pid_file: Option<P>,
    notify_socket: &mut NotifyListener,
    rootfs: PathBuf,
    spec: spec::Spec,
    csocketfd: Option<FileDescriptor>,
    container: Container,
    command: impl Command,
) -> Result<Process> {
    prctl::set_dumpable(false).unwrap();
    let linux = spec.linux.as_ref().unwrap();
    let namespaces: Namespaces = linux.namespaces.clone().into();

    let cmanager = cgroups::Manager::new(linux.cgroups_path.clone())?;

    match fork::fork_first(
        pid_file,
        namespaces
            .clone_flags
            .contains(sched::CloneFlags::CLONE_NEWUSER),
        linux,
        &container,
        &cmanager,
    )? {
        Process::Parent(parent) => Ok(Process::Parent(parent)),
        Process::Child(child) => {
            for rlimit in spec.process.rlimits.iter() {
                command.set_rlimit(rlimit)?
            }
            command.set_id(Uid::from_raw(0), Gid::from_raw(0))?;

            let without = sched::CloneFlags::CLONE_NEWUSER;
            namespaces.apply_unshare(without)?;

            if let Some(csocketfd) = csocketfd {
                tty::ready(csocketfd)?;
            }

            namespaces.apply_setns()?;

            match fork::fork_init(child)? {
                Process::Child(child) => Ok(Process::Child(child)),
                Process::Init(mut init) => {
                    let spec_args: &Vec<String> = &spec.process.args.clone();
                    let envs: &Vec<String> = &spec.process.env.clone();
                    init_process(spec, command, rootfs, namespaces)?;
                    init.ready()?;
                    notify_socket.wait_for_container_start()?;

                    utils::do_exec(&spec_args[0], spec_args, envs)?;
                    container.update_status(ContainerStatus::Stopped)?.save()?;

                    Ok(Process::Init(init))
                }
                Process::Parent(_) => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}

fn init_process(
    spec: spec::Spec,
    command: impl Command,
    rootfs: PathBuf,
    namespaces: Namespaces,
) -> Result<()> {
    let proc = spec.process.clone();
    let clone_spec = std::sync::Arc::new(spec);
    let clone_rootfs = std::sync::Arc::new(rootfs.clone());

    command.set_hostname(&clone_spec.hostname.as_str())?;
    if clone_spec.process.no_new_privileges {
        let _ = prctl::set_no_new_privileges(true);
    }

    futures::executor::block_on(rootfs::prepare_rootfs(
        clone_spec,
        clone_rootfs,
        namespaces
            .clone_flags
            .contains(sched::CloneFlags::CLONE_NEWUSER),
    ))?;

    command.pivot_rootfs(&rootfs)?;

    command.set_id(Uid::from_raw(proc.user.uid), Gid::from_raw(proc.user.gid))?;
    capabilities::reset_effective(&command)?;
    if let Some(caps) = &proc.capabilities {
        capabilities::drop_privileges(&caps, &command)?;
    }
    Ok(())
}
