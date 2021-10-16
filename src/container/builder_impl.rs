use super::{Container, ContainerStatus};
use crate::{
    hooks,
    notify_socket::NotifyListener,
    process::{self, args::ContainerArgs},
    rootless::Rootless,
    syscall::Syscall,
    utils,
};
use anyhow::{bail, Context, Result};
use oci_spec::runtime::Spec;
use std::{fs, io::Write, os::unix::prelude::RawFd, path::PathBuf};

pub(super) struct ContainerBuilderImpl<'a> {
    /// Flag indicating if an init or a tenant container should be created
    pub init: bool,
    /// Interface to operating system primitives
    pub syscall: &'a dyn Syscall,
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
        if let Err(outer) = self.run_container().context("failed to create container") {
            if let Err(inner) = self.cleanup_container() {
                return Err(outer.context(inner));
            }

            return Err(outer);
        }

        Ok(())
    }

    fn run_container(&mut self) -> Result<()> {
        let linux = self.spec.linux().as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(linux.cgroups_path(), &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;
        let process = self.spec.process().as_ref().context("No process in spec")?;

        if self.init {
            if let Some(hooks) = self.spec.hooks() {
                hooks::run_hooks(hooks.create_runtime().as_ref(), self.container.as_ref())?
            }
        }

        // Need to create the notify socket before we pivot root, since the unix
        // domain socket used here is outside of the rootfs of container. During
        // exec, need to create the socket before we enter into existing mount
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
        if let Some(oom_score_adj) = process.oom_score_adj() {
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
        if linux.namespaces().is_some() {
            prctl::set_dumpable(false).unwrap();
        }

        // This intermediate_args will be passed to the container intermediate process,
        // therefore we will have to move all the variable by value. Since self
        // is a shared reference, we have to clone these variables here.
        let container_args = ContainerArgs {
            init: self.init,
            syscall: self.syscall,
            spec: self.spec,
            rootfs: &self.rootfs,
            console_socket: self.console_socket,
            notify_socket,
            preserve_fds: self.preserve_fds,
            container: &self.container,
            rootless: &self.rootless,
            cgroup_manager: cmanager,
        };

        let init_pid = process::container_main_process::container_main_process(&container_args)?;

        // if file to write the pid to is specified, write pid of the child
        if let Some(pid_file) = &self.pid_file {
            fs::write(&pid_file, format!("{}", init_pid)).context("failed to write pid file")?;
        }

        if let Some(container) = &mut self.container {
            // update status and pid of the container process
            container
                .set_status(ContainerStatus::Created)
                .set_creator(nix::unistd::geteuid().as_raw())
                .set_pid(init_pid.as_raw())
                .save()
                .context("Failed to save container state")?;
        }

        Ok(())
    }

    fn cleanup_container(&self) -> Result<()> {
        let linux = self.spec.linux().as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(linux.cgroups_path(), &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;

        let mut errors = Vec::new();
        if let Err(e) = cmanager.remove().context("failed to remove cgroup") {
            errors.push(e.to_string());
        }

        if let Some(container) = &self.container {
            if container.root.exists() {
                if let Err(e) = fs::remove_dir_all(&container.root)
                    .with_context(|| format!("could not delete {:?}", container.root))
                {
                    errors.push(e.to_string());
                }
            }
        }

        if !errors.is_empty() {
            bail!("failed to cleanup container: {}", errors.join(";"));
        }

        Ok(())
    }
}
