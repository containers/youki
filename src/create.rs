use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Result};
use clap::Clap;
use nix::fcntl;
use nix::sched;
use nix::sys::stat;
use nix::unistd;
use nix::unistd::{Gid, Uid};

use crate::container::{Container, ContainerStatus};
use crate::notify_socket::NotifyListener;
use crate::process::{fork, Process};
use crate::rootfs;
use crate::spec;
use crate::stdio::FileDescriptor;
use crate::tty;
use crate::utils;

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
    pub fn exec(&self, root_path: PathBuf) -> Result<()> {
        let container_dir = root_path.join(&self.container_id);
        if !container_dir.exists() {
            fs::create_dir(&container_dir).unwrap();
        } else {
            bail!("{} already exists", self.container_id)
        }

        unistd::chdir(&self.bundle)?;

        let spec = spec::Spec::load("config.json")?;

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
) -> Result<Process> {
    prctl::set_dumpable(false).unwrap();
    let linux = spec.linux.as_ref().unwrap();

    let mut cf = sched::CloneFlags::empty();
    let mut to_enter = Vec::new();
    for ns in &linux.namespaces {
        let space = sched::CloneFlags::from_bits_truncate(ns.typ as i32);
        if ns.path.is_empty() {
            cf |= space;
        } else {
            let fd = fcntl::open(&*ns.path, fcntl::OFlag::empty(), stat::Mode::empty()).unwrap();
            to_enter.push((space, fd));
        }
    }

    match fork::fork_first(
        pid_file,
        cf.contains(sched::CloneFlags::CLONE_NEWUSER),
        linux,
        &container,
    )? {
        Process::Parent(parent) => Ok(Process::Parent(parent)),
        Process::Child(child) => {
            sched::unshare(cf & !sched::CloneFlags::CLONE_NEWUSER)?;

            if let Some(csocketfd) = csocketfd {
                tty::ready(csocketfd)?;
            }

            for &(space, fd) in &to_enter {
                sched::setns(fd, space)?;
                unistd::close(fd)?;
                if space == sched::CloneFlags::CLONE_NEWUSER {
                    setid(Uid::from_raw(0), Gid::from_raw(0))?;
                }
            }

            match fork::fork_init(child)? {
                Process::Child(child) => Ok(Process::Child(child)),
                Process::Init(mut init) => {
                    let spec_args: &Vec<String> = &spec.process.args.clone();

                    let clone_spec = std::sync::Arc::new(spec);
                    let clone_rootfs = std::sync::Arc::new(rootfs.clone());

                    futures::executor::block_on(rootfs::prepare_rootfs(
                        clone_spec,
                        clone_rootfs,
                        cf.contains(sched::CloneFlags::CLONE_NEWUSER),
                    ))?;

                    rootfs::pivot_rootfs(&rootfs)?;

                    init.ready()?;

                    notify_socket.wait_for_container_start()?;

                    // utils::do_exec(&spec.process.args[0], &spec.process.args)?;
                    utils::do_exec(&spec_args[0], spec_args)?;
                    container.update_status(ContainerStatus::Stopped)?.save()?;

                    Ok(Process::Init(init))
                }
                Process::Parent(_) => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}

fn setid(uid: Uid, gid: Gid) -> Result<()> {
    if let Err(e) = prctl::set_keep_capabilities(true) {
        bail!("set keep capabilities returned {}", e);
    };
    unistd::setresgid(gid, gid, gid)?;
    unistd::setresuid(uid, uid, uid)?;
    if let Err(e) = prctl::set_keep_capabilities(false) {
        bail!("set keep capabilities returned {}", e);
    };
    Ok(())
}
