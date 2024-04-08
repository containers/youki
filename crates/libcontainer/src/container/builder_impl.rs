use super::{stdio::StdioFds, Container, ContainerStatus};
use crate::{
    error::{LibcontainerError, MissingSpecError},
    hooks,
    notify_socket::NotifyListener,
    pipe::{Pipe, PipeError, PipeHolder},
    process::{
        self,
        args::{ContainerArgs, ContainerType},
        intel_rdt::delete_resctrl_subdirectory,
    },
    stdio::{Closing, Fd},
    syscall::syscall::SyscallType,
    user_ns::UserNamespaceConfig,
    utils,
    workload::Executor,
};
use libcgroups::common::CgroupManager;
use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    sys::stat::Mode,
    unistd::Pid,
};
use oci_spec::runtime::Spec;
use std::{
    collections::HashMap,
    fs,
    io::Write,
    os::{fd::AsRawFd, unix::prelude::RawFd},
    path::{Path, PathBuf},
    rc::Rc,
};

pub(super) struct ContainerBuilderImpl {
    /// Flag indicating if an init or a tenant container should be created
    pub container_type: ContainerType,
    /// Interface to operating system primitives
    pub syscall: SyscallType,
    /// Flag indicating if systemd should be used for cgroup management
    pub use_systemd: bool,
    /// Id of the container
    pub container_id: String,
    /// OCI compliant runtime spec
    pub spec: Rc<Spec>,
    /// Root filesystem of the container
    pub rootfs: PathBuf,
    /// File which will be used to communicate the pid of the
    /// container process to the higher level runtime
    pub pid_file: Option<PathBuf>,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// Options for new user namespace
    pub user_ns_config: Option<UserNamespaceConfig>,
    /// Path to the Unix Domain Socket to communicate container start
    pub notify_path: PathBuf,
    /// Container state
    pub container: Option<Container>,
    /// File descriptos preserved/passed to the container init process.
    pub preserve_fds: i32,
    /// If the container is to be run in detached mode
    pub detached: bool,
    /// Default executes the specified execution of a generic command
    pub executor: Box<dyn Executor>,
    /// Stdio file descriptors to dup inside the container's namespace
    pub fds: [Fd; 3],
}

impl ContainerBuilderImpl {
    pub(super) fn create(&mut self) -> Result<(Pid, StdioFds), LibcontainerError> {
        match self.run_container() {
            Ok(ret) => Ok(ret),
            Err(outer) => {
                // Only the init container should be cleaned up in the case of
                // an error.
                if matches!(self.container_type, ContainerType::InitContainer) {
                    self.cleanup_container()?;
                }

                Err(outer)
            }
        }
    }

    fn run_container(&mut self) -> Result<(Pid, StdioFds), LibcontainerError> {
        let linux = self.spec.linux().as_ref().ok_or(MissingSpecError::Linux)?;
        let cgroups_path = utils::get_cgroup_path(
            linux.cgroups_path(),
            &self.container_id,
            self.user_ns_config.is_some(),
        );
        let cgroup_config = libcgroups::common::CgroupConfig {
            cgroup_path: cgroups_path,
            systemd_cgroup: self.use_systemd || self.user_ns_config.is_some(),
            container_name: self.container_id.to_owned(),
        };
        let process = self
            .spec
            .process()
            .as_ref()
            .ok_or(MissingSpecError::Process)?;

        if matches!(self.container_type, ContainerType::InitContainer) {
            if let Some(hooks) = self.spec.hooks() {
                hooks::run_hooks(hooks.create_runtime().as_ref(), self.container.as_ref())?
            }
        }

        // Need to create the notify socket before we pivot root, since the unix
        // domain socket used here is outside of the rootfs of container. During
        // exec, need to create the socket before we enter into existing mount
        // namespace. We also need to create to socket before entering into the
        // user namespace in the case that the path is located in paths only
        // root can access.
        let notify_listener = NotifyListener::new(&self.notify_path)?;

        // If Out-of-memory score adjustment is set in specification.  set the score
        // value for the current process check
        // https://dev.to/rrampage/surviving-the-linux-oom-killer-2ki9 for some more
        // information.
        //
        // This has to be done before !dumpable because /proc/self/oom_score_adj
        // is not writeable unless you're an privileged user (if !dumpable is
        // set). All children inherit their parent's oom_score_adj value on
        // fork(2) so this will always be propagated properly.
        if let Some(oom_score_adj) = process.oom_score_adj() {
            tracing::debug!("Set OOM score to {}", oom_score_adj);
            let mut f = fs::File::create("/proc/self/oom_score_adj").map_err(|err| {
                tracing::error!("failed to open /proc/self/oom_score_adj: {}", err);
                LibcontainerError::OtherIO(err)
            })?;
            f.write_all(oom_score_adj.to_string().as_bytes())
                .map_err(|err| {
                    tracing::error!("failed to write to /proc/self/oom_score_adj: {}", err);
                    LibcontainerError::OtherIO(err)
                })?;
        }

        // Make the process non-dumpable, to avoid various race conditions that
        // could cause processes in namespaces we're joining to access host
        // resources (or potentially execute code).
        //
        // However, if the number of namespaces we are joining is 0, we are not
        // going to be switching to a different security context. Thus setting
        // ourselves to be non-dumpable only breaks things (like rootless
        // containers), which is the recommendation from the kernel folks.
        if linux.namespaces().is_some() {
            prctl::set_dumpable(false).map_err(|e| {
                LibcontainerError::Other(format!(
                    "error in setting dumpable to false : {}",
                    nix::errno::from_i32(e)
                ))
            })?;
        }

        // Prepare the stdio file descriptors for `dup`-ing inside the container
        // namespace. Determines which ones needs closing on drop.
        let mut stdio_descs = prepare_stdio_descriptors(&self.fds)?;
        // Extract `StdioFds` from the prepared fds, for use by client
        let stdio_fds = (&mut stdio_descs).into();

        // This container_args will be passed to the container processes,
        // therefore we will have to move all the variable by value. Since self
        // is a shared reference, we have to clone these variables here.
        let container_args = ContainerArgs {
            container_type: self.container_type,
            syscall: self.syscall,
            spec: Rc::clone(&self.spec),
            rootfs: self.rootfs.to_owned(),
            console_socket: self.console_socket,
            notify_listener,
            preserve_fds: self.preserve_fds,
            container: self.container.to_owned(),
            user_ns_config: self.user_ns_config.to_owned(),
            cgroup_config,
            detached: self.detached,
            executor: self.executor.clone(),
            fds: stdio_descs.inner,
        };

        let (init_pid, need_to_clean_up_intel_rdt_dir) =
            process::container_main_process::container_main_process(&container_args).map_err(
                |err| {
                    tracing::error!(?err, "failed to run container process");
                    LibcontainerError::MainProcess(err)
                },
            )?;

        // if file to write the pid to is specified, write pid of the child
        if let Some(pid_file) = &self.pid_file {
            fs::write(pid_file, format!("{init_pid}")).map_err(|err| {
                tracing::error!("failed to write pid to file: {}", err);
                LibcontainerError::OtherIO(err)
            })?;
        }

        if let Some(container) = &mut self.container {
            // update status and pid of the container process
            container
                .set_status(ContainerStatus::Created)
                .set_creator(nix::unistd::geteuid().as_raw())
                .set_pid(init_pid.as_raw())
                .set_clean_up_intel_rdt_directory(need_to_clean_up_intel_rdt_dir)
                .save()?;
        }

        Ok((init_pid, stdio_fds))
    }

    fn cleanup_container(&self) -> Result<(), LibcontainerError> {
        let linux = self.spec.linux().as_ref().ok_or(MissingSpecError::Linux)?;
        let cgroups_path = utils::get_cgroup_path(
            linux.cgroups_path(),
            &self.container_id,
            self.user_ns_config.is_some(),
        );
        let cmanager =
            libcgroups::common::create_cgroup_manager(libcgroups::common::CgroupConfig {
                cgroup_path: cgroups_path,
                systemd_cgroup: self.use_systemd || self.user_ns_config.is_some(),
                container_name: self.container_id.to_string(),
            })?;

        let mut errors = Vec::new();

        if let Err(e) = cmanager.remove() {
            tracing::error!(error = ?e, "failed to remove cgroup manager");
            errors.push(e.to_string());
        }

        if let Some(container) = &self.container {
            if let Some(true) = container.clean_up_intel_rdt_subdirectory() {
                if let Err(e) = delete_resctrl_subdirectory(container.id()) {
                    tracing::error!(id = ?container.id(), error = ?e, "failed to delete resctrl subdirectory");
                    errors.push(e.to_string());
                }
            }

            if container.root.exists() {
                if let Err(e) = fs::remove_dir_all(&container.root) {
                    tracing::error!(container_root = ?container.root, error = ?e, "failed to delete container root");
                    errors.push(e.to_string());
                }
            }
        }

        if !errors.is_empty() {
            return Err(LibcontainerError::Other(format!(
                "failed to cleanup container: {}",
                errors.join(";")
            )));
        }

        Ok(())
    }
}

struct StdioDescriptors {
    inner: HashMap<RawFd, RawFd>,
    outer: HashMap<RawFd, PipeHolder>,
    _guards: Vec<Closing>,
}

impl From<&mut StdioDescriptors> for StdioFds {
    fn from(value: &mut StdioDescriptors) -> Self {
        StdioFds {
            stdin: value.outer.remove(&0).and_then(|x| match x {
                PipeHolder::Writer(x) => Some(x),
                _ => None,
            }),
            stdout: value.outer.remove(&1).and_then(|x| match x {
                PipeHolder::Reader(x) => Some(x),
                _ => None,
            }),
            stderr: value.outer.remove(&2).and_then(|x| match x {
                PipeHolder::Reader(x) => Some(x),
                _ => None,
            }),
        }
    }
}

fn prepare_stdio_descriptors(fds: &[Fd; 3]) -> Result<StdioDescriptors, LibcontainerError> {
    let mut inner = HashMap::new();
    let mut outer = HashMap::new();
    let mut guards = Vec::new();
    for (idx, fdkind) in fds.iter().enumerate() {
        let dest_fd = idx as i32;
        let mut fd = match fdkind {
            Fd::ReadPipe => {
                let (rd, wr) = Pipe::new()?.split();
                let fd = rd.into_fd();
                guards.push(Closing::new(fd));
                outer.insert(dest_fd, PipeHolder::Writer(wr));
                fd
            }
            Fd::WritePipe => {
                let (rd, wr) = Pipe::new()?.split();
                let fd = wr.into_fd();
                guards.push(Closing::new(fd));
                outer.insert(dest_fd, PipeHolder::Reader(rd));
                fd
            }
            Fd::ReadNull => {
                // Need to keep fd with cloexec, until we are in child
                let fd = nix::fcntl::open(
                    Path::new("/dev/null"),
                    OFlag::O_CLOEXEC | OFlag::O_RDONLY,
                    Mode::empty(),
                )
                .map_err(PipeError::Open)?;
                guards.push(Closing::new(fd));
                fd
            }
            Fd::WriteNull => {
                // Need to keep fd with cloexec, until we are in child
                let fd = nix::fcntl::open(
                    Path::new("/dev/null"),
                    OFlag::O_CLOEXEC | OFlag::O_WRONLY,
                    Mode::empty(),
                )
                .map_err(PipeError::Open)?;
                guards.push(Closing::new(fd));
                fd
            }
            Fd::Inherit => dest_fd,
            Fd::Fd(ref x) => x.as_raw_fd(),
        };
        // The descriptor must not clobber the descriptors that are passed to
        // a child
        while fd != dest_fd && fd < 3 {
            fd = fcntl(fd, FcntlArg::F_DUPFD_CLOEXEC(3)).map_err(PipeError::Dup)?;
            guards.push(Closing::new(fd));
        }
        inner.insert(dest_fd, fd);
    }
    Ok(StdioDescriptors {
        inner,
        outer,
        _guards: guards,
    })
}
