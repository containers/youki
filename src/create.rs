//! Handles the creation of a new container
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Result};
use clap::Clap;
use nix::sched;
use nix::unistd;
use nix::unistd::{Gid, Uid};

use crate::container::{Container, ContainerStatus};
use crate::namespaces::Namespaces;
use crate::notify_socket::NotifyListener;
use crate::process::{fork, Process};
use crate::rootfs;
use crate::rootless::{lookup_map_binaries, should_use_rootless, Rootless};
use crate::stdio::FileDescriptor;
use crate::tty;
use crate::utils;
use crate::{capabilities, command::Command};
use crate::{cgroups, rootless};

/// This is the main structure which stores various commandline options given by
/// high-level container runtime
#[derive(Clap, Debug)]
pub struct Create {
    /// File to write pid of the container created
    // note that in the end, container is just another process
    #[clap(short, long)]
    pid_file: Option<String>,
    /// path to the bundle directory, containing config.json and root filesystem
    #[clap(short, long, default_value = ".")]
    bundle: PathBuf,
    /// Unix socket (file) path , which will receive file descriptor of the writing end of the pseudoterminal
    #[clap(short, long)]
    console_socket: Option<PathBuf>,
    /// name of the container instance to be started
    pub container_id: String,
}

// One thing to note is that in the end, container is just another process in Linux
// it has specific/different control group, namespace, using which program executing in it
// can be given impression that is is running on a complete system, but on the system which
// it is running, it is just another process, and has attributes such as pid, file descriptors, etc.
// associated with it like any other process.
impl Create {
    /// Starts a new container process
    pub fn exec(
        &self,
        root_path: PathBuf,
        systemd_cgroup: bool,
        command: impl Command,
    ) -> Result<()> {
        // create a directory for the container to store state etc.
        // if already present, return error
        let bundle_canonicalized = fs::canonicalize(&self.bundle)
            .unwrap_or_else(|_| panic!("failed to canonicalied {:?}", &self.bundle));
        let container_dir = root_path.join(&self.container_id);
        log::debug!("container directory will be {:?}", container_dir);

        if !container_dir.exists() {
            fs::create_dir(&container_dir).unwrap();
        } else {
            bail!("{} already exists", self.container_id)
        }

        // change directory to the bundle directory, and load configuration,
        // copy that to the container's directory
        unistd::chdir(&self.bundle)?;

        let spec = oci_spec::Spec::load("config.json")?;
        fs::copy("config.json", container_dir.join("config.json"))?;
        log::debug!("spec: {:?}", spec);

        // convert path to absolute path, as relative path will be evaluated
        // relative to where youki command is executed, and will be difficult to manipulate
        let container_dir = fs::canonicalize(container_dir)?;
        unistd::chdir(&*container_dir)?;

        log::debug!("{:?}", &container_dir);

        let container = Container::new(
            &self.container_id,
            ContainerStatus::Creating,
            None,
            bundle_canonicalized.to_str().unwrap(),
            &container_dir,
        )?;
        container.save()?;

        let mut notify_socket: NotifyListener = NotifyListener::new(&container_dir)?;
        // convert path of root file system of the container to absolute path
        let rootfs = fs::canonicalize(&spec.root.path)?;

        // if socket file path is given in commandline options,
        // get file descriptors of console socket
        let csocketfd = if let Some(console_socket) = &self.console_socket {
            Some(tty::setup_console_socket(&container_dir, console_socket)?)
        } else {
            None
        };

        let process = run_container(
            self.pid_file.as_ref(),
            &mut notify_socket,
            rootfs,
            spec,
            csocketfd,
            systemd_cgroup,
            container,
            command,
        )?;
        // the run_container forks the process, so not after return if in
        // parent process, exit ;  as the work of creating the container is done
        if let Process::Parent(_) = process {
            process::exit(0);
        }
        // if in the child process after fork, then just return
        Ok(())
    }
}

/// Fork the process and actually start the container process
fn run_container<P: AsRef<Path>>(
    pid_file: Option<P>,
    notify_socket: &mut NotifyListener,
    rootfs: PathBuf,
    spec: oci_spec::Spec,
    csocketfd: Option<FileDescriptor>,
    systemd_cgroup: bool,
    container: Container,
    command: impl Command,
) -> Result<Process> {
    // disable core dump for the process, check https://man7.org/linux/man-pages/man2/prctl.2.html for more information
    prctl::set_dumpable(false).unwrap();

    // get Linux specific section of OCI spec,
    // refer https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md for more information
    let linux = spec.linux.as_ref().unwrap();
    let namespaces: Namespaces = linux.namespaces.clone().into();

    let rootless = if should_use_rootless() {
        log::debug!("rootless container should be created");
        log::warn!(
            "resource constraints and multi id mapping is unimplemented for rootless containers"
        );
        rootless::validate(&spec)?;
        let mut rootless = Rootless::from(linux);
        if let Some((uid_binary, gid_binary)) = lookup_map_binaries(linux)? {
            rootless.newuidmap = Some(uid_binary);
            rootless.newgidmap = Some(gid_binary);
        }
        Some(rootless)
    } else {
        None
    };

    let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, container.id());
    let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, systemd_cgroup)?;

    // first fork, which creates process, which will later create actual container process
    match fork::fork_first(pid_file, rootless, linux, &container, cmanager)? {
        // In the parent process, which called run_container
        Process::Parent(parent) => Ok(Process::Parent(parent)),
        // in child process
        Process::Child(child) => {
            // set limits and namespaces to the process
            for rlimit in spec.process.rlimits.iter() {
                command.set_rlimit(rlimit)?
            }
            command.set_id(Uid::from_raw(0), Gid::from_raw(0))?;

            let without = sched::CloneFlags::CLONE_NEWUSER;
            namespaces.apply_unshare(without)?;

            // set up tty if specified
            if let Some(csocketfd) = csocketfd {
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
                    setup_init_process(&spec, command, rootfs, &namespaces)?;
                    init.ready()?;
                    notify_socket.wait_for_container_start()?;
                    // actually run the command / program to be run in container
                    let args: &Vec<String> = &spec.process.args;
                    let envs: &Vec<String> = &spec.process.env;
                    utils::do_exec(&args[0], args, envs)?;

                    // the command / program is done executing
                    container
                        .refresh_state()?
                        .update_status(ContainerStatus::Stopped)
                        .save()?;

                    Ok(Process::Init(init))
                }
                Process::Parent(_) => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}

/// setup hostname, rootfs for the container process
fn setup_init_process(
    spec: &oci_spec::Spec,
    command: impl Command,
    rootfs: PathBuf,
    namespaces: &Namespaces,
) -> Result<()> {
    let proc = &spec.process;

    command.set_hostname(spec.hostname.as_str())?;
    if proc.no_new_privileges {
        let _ = prctl::set_no_new_privileges(true);
    }

    rootfs::prepare_rootfs(
        &spec,
        &rootfs,
        namespaces
            .clone_flags
            .contains(sched::CloneFlags::CLONE_NEWUSER),
    )?;

    // change the root of filesystem of the process to the rootfs
    command.pivot_rootfs(&rootfs)?;

    command.set_id(Uid::from_raw(proc.user.uid), Gid::from_raw(proc.user.gid))?;
    capabilities::reset_effective(&command)?;
    if let Some(caps) = &proc.capabilities {
        capabilities::drop_privileges(&caps, &command)?;
    }
    Ok(())
}
