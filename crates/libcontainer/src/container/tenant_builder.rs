use anyhow::{bail, Context, Result};
use caps::Capability;
use nix::fcntl::OFlag;
use nix::unistd::{self, close, pipe2, read, Pid};
use oci_spec::runtime::{
    Capabilities as SpecCapabilities, Capability as SpecCapability, LinuxBuilder,
    LinuxCapabilities, LinuxCapabilitiesBuilder, LinuxNamespace, LinuxNamespaceBuilder,
    LinuxNamespaceType, Process, ProcessBuilder, Spec,
};
use procfs::process::Namespace;

use std::{
    collections::HashMap,
    convert::TryFrom,
    ffi::{OsStr, OsString},
    fs,
    os::unix::prelude::RawFd,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::process::args::ContainerType;
use crate::{capabilities::CapabilityExt, container::builder_impl::ContainerBuilderImpl};
use crate::{notify_socket::NotifySocket, rootless::Rootless, tty, utils};

use super::{builder::ContainerBuilder, Container};

const NAMESPACE_TYPES: &[&str] = &["ipc", "uts", "net", "pid", "mnt", "cgroup"];
const TENANT_NOTIFY: &str = "tenant-notify-";
const TENANT_TTY: &str = "tenant-tty-";

/// Builder that can be used to configure the properties of a process
/// that will join an existing container sandbox
pub struct TenantContainerBuilder<'a> {
    base: ContainerBuilder<'a>,
    env: HashMap<String, String>,
    cwd: Option<PathBuf>,
    args: Vec<String>,
    no_new_privs: Option<bool>,
    capabilities: Vec<String>,
    process: Option<PathBuf>,
    detached: bool,
}

impl<'a> TenantContainerBuilder<'a> {
    /// Generates the base configuration for a process that will join
    /// an existing container sandbox from which configuration methods
    /// can be chained
    pub(super) fn new(builder: ContainerBuilder<'a>) -> Self {
        Self {
            base: builder,
            env: HashMap::new(),
            cwd: None,
            args: Vec::new(),
            no_new_privs: None,
            capabilities: Vec::new(),
            process: None,
            detached: false,
        }
    }

    /// Sets environment variables for the container
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Sets the working directory of the container
    pub fn with_cwd<P: Into<PathBuf>>(mut self, path: Option<P>) -> Self {
        self.cwd = path.map(|p| p.into());
        self
    }

    /// Sets the command the container will be started with
    pub fn with_container_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_no_new_privs(mut self, no_new_privs: bool) -> Self {
        self.no_new_privs = Some(no_new_privs);
        self
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_process<P: Into<PathBuf>>(mut self, path: Option<P>) -> Self {
        self.process = path.map(|p| p.into());
        self
    }

    pub fn with_detach(mut self, detached: bool) -> Self {
        self.detached = detached;
        self
    }

    /// Joins an existing container
    pub fn build(self) -> Result<Pid> {
        let container_dir = self
            .lookup_container_dir()
            .context("failed to look up container dir")?;
        let container = self
            .load_container_state(container_dir.clone())
            .context("failed to load container state")?;
        let mut spec = self
            .load_init_spec(&container)
            .context("failed to load init spec")?;
        self.adapt_spec_for_tenant(&mut spec, &container)
            .context("failed to adapt spec for tenant")?;

        log::debug!("{:#?}", spec);

        unistd::chdir(&container_dir)?;
        let notify_path = Self::setup_notify_listener(&container_dir)?;
        // convert path of root file system of the container to absolute path
        let rootfs = fs::canonicalize(spec.root().as_ref().context("no root in spec")?.path())?;

        // if socket file path is given in commandline options,
        // get file descriptors of console socket
        let csocketfd = self.setup_tty_socket(&container_dir)?;

        let use_systemd = self.should_use_systemd(&container);
        let rootless = Rootless::new(&spec)?;

        let (read_end, write_end) = pipe2(OFlag::O_CLOEXEC)?;

        let mut builder_impl = ContainerBuilderImpl {
            container_type: ContainerType::TenantContainer {
                exec_notify_fd: write_end,
            },
            syscall: self.base.syscall,
            container_id: self.base.container_id,
            pid_file: self.base.pid_file,
            console_socket: csocketfd,
            use_systemd,
            spec: &spec,
            rootfs,
            rootless,
            notify_path: notify_path.clone(),
            container: None,
            preserve_fds: self.base.preserve_fds,
            detached: self.detached,
        };

        let pid = builder_impl.create()?;

        let mut notify_socket = NotifySocket::new(notify_path);
        notify_socket.notify_container_start()?;

        close(write_end)?;

        let mut err_str_buf = Vec::new();

        loop {
            let mut buf = [0; 3];
            match read(read_end, &mut buf)? {
                0 => {
                    if err_str_buf.is_empty() {
                        return Ok(pid);
                    } else {
                        bail!(String::from_utf8_lossy(&err_str_buf).to_string());
                    }
                }
                _ => {
                    err_str_buf.extend(buf.into_iter());
                }
            }
        }
    }

    fn lookup_container_dir(&self) -> Result<PathBuf> {
        let container_dir = self.base.root_path.join(&self.base.container_id);
        if !container_dir.exists() {
            bail!("container {} does not exist", self.base.container_id);
        }

        Ok(container_dir)
    }

    fn load_init_spec(&self, container: &Container) -> Result<Spec> {
        let spec_path = container.bundle().join("config.json");

        let mut spec = Spec::load(&spec_path)
            .with_context(|| format!("failed to load spec from {spec_path:?}"))?;

        spec.canonicalize_rootfs(container.bundle())
            .context("failed to canonicalize rootfs")?;
        Ok(spec)
    }

    fn load_container_state(&self, container_dir: PathBuf) -> Result<Container> {
        let container = Container::load(container_dir)?;
        if !container.can_exec() {
            bail!(
                "Cannot exec as container is in state {}",
                container.status()
            );
        }

        Ok(container)
    }

    fn adapt_spec_for_tenant(&self, spec: &mut Spec, container: &Container) -> Result<()> {
        let process = if let Some(process) = &self.process {
            self.get_process(process)?
        } else {
            let mut process_builder = ProcessBuilder::default()
                .args(self.get_args()?)
                .env(self.get_environment()?);
            if let Some(cwd) = self.get_working_dir()? {
                process_builder = process_builder.cwd(cwd);
            }

            if let Some(no_new_priv) = self.get_no_new_privileges() {
                process_builder = process_builder.no_new_privileges(no_new_priv);
            }

            if let Some(caps) = self.get_capabilities(spec)? {
                process_builder = process_builder.capabilities(caps);
            }

            process_builder.build()?
        };

        if container.pid().is_none() {
            bail!("could not retrieve container init pid");
        }

        let init_process = procfs::process::Process::new(container.pid().unwrap().as_raw())?;
        let ns = self.get_namespaces(init_process.namespaces()?)?;
        let linux = LinuxBuilder::default().namespaces(ns).build()?;

        spec.set_process(Some(process)).set_linux(Some(linux));
        Ok(())
    }

    fn get_process(&self, process: &Path) -> Result<Process> {
        if !process.exists() {
            bail!(
                "Process.json file does not exist at specified path {}",
                process.display()
            )
        }

        let process = utils::open(process)?;
        let process_spec = serde_json::from_reader(process)?;
        Ok(process_spec)
    }

    fn get_working_dir(&self) -> Result<Option<PathBuf>> {
        if let Some(cwd) = &self.cwd {
            if cwd.is_relative() {
                bail!(
                    "current working directory must be an absolute path, but is {:?}",
                    cwd
                );
            }
            return Ok(Some(cwd.into()));
        }
        Ok(None)
    }

    fn get_args(&self) -> Result<Vec<String>> {
        if self.args.is_empty() {
            bail!("container command was not specified")
        }

        Ok(self.args.clone())
    }

    fn get_environment(&self) -> Result<Vec<String>> {
        Ok(self.env.iter().map(|(k, v)| format!("{k}={v}")).collect())
    }

    fn get_no_new_privileges(&self) -> Option<bool> {
        self.no_new_privs
    }

    fn get_capabilities(&self, spec: &Spec) -> Result<Option<LinuxCapabilities>> {
        if !self.capabilities.is_empty() {
            let mut caps: Vec<Capability> = Vec::with_capacity(self.capabilities.len());
            for cap in &self.capabilities {
                caps.push(Capability::from_str(cap)?);
            }

            let caps: SpecCapabilities =
                caps.iter().map(|c| SpecCapability::from_cap(*c)).collect();

            if let Some(spec_caps) = spec
                .process()
                .as_ref()
                .context("no process in spec")?
                .capabilities()
            {
                let mut capabilities_builder = LinuxCapabilitiesBuilder::default();
                capabilities_builder = match spec_caps.ambient() {
                    Some(ambient) => {
                        let ambient: SpecCapabilities = ambient.union(&caps).copied().collect();
                        capabilities_builder.ambient(ambient)
                    }
                    None => capabilities_builder,
                };
                capabilities_builder = match spec_caps.bounding() {
                    Some(bounding) => {
                        let bounding: SpecCapabilities = bounding.union(&caps).copied().collect();
                        capabilities_builder.bounding(bounding)
                    }
                    None => capabilities_builder,
                };
                capabilities_builder = match spec_caps.effective() {
                    Some(effective) => {
                        let effective: SpecCapabilities = effective.union(&caps).copied().collect();
                        capabilities_builder.effective(effective)
                    }
                    None => capabilities_builder,
                };
                capabilities_builder = match spec_caps.inheritable() {
                    Some(inheritable) => {
                        let inheritable: SpecCapabilities =
                            inheritable.union(&caps).copied().collect();
                        capabilities_builder.inheritable(inheritable)
                    }
                    None => capabilities_builder,
                };
                capabilities_builder = match spec_caps.permitted() {
                    Some(permitted) => {
                        let permitted: SpecCapabilities = permitted.union(&caps).copied().collect();
                        capabilities_builder.permitted(permitted)
                    }
                    None => capabilities_builder,
                };

                let c = capabilities_builder.build()?;
                return Ok(Some(c));
            }

            return Ok(Some(
                LinuxCapabilitiesBuilder::default()
                    .bounding(caps.clone())
                    .effective(caps.clone())
                    .inheritable(caps.clone())
                    .permitted(caps.clone())
                    .ambient(caps)
                    .build()?,
            ));
        }

        Ok(None)
    }

    fn get_namespaces(
        &self,
        init_namespaces: HashMap<OsString, Namespace>,
    ) -> Result<Vec<LinuxNamespace>> {
        let mut tenant_namespaces = Vec::with_capacity(init_namespaces.len());

        for &ns_type in NAMESPACE_TYPES {
            if let Some(init_ns) = init_namespaces.get(OsStr::new(ns_type)) {
                let tenant_ns = LinuxNamespaceType::try_from(ns_type)?;
                tenant_namespaces.push(
                    LinuxNamespaceBuilder::default()
                        .typ(tenant_ns)
                        .path(init_ns.path.clone())
                        .build()?,
                )
            }
        }

        Ok(tenant_namespaces)
    }

    fn should_use_systemd(&self, container: &Container) -> bool {
        if let Some(use_systemd) = container.systemd() {
            return use_systemd;
        }

        false
    }

    fn setup_notify_listener(container_dir: &Path) -> Result<PathBuf> {
        let notify_name = Self::generate_name(container_dir, TENANT_NOTIFY);
        let socket_path = container_dir.join(notify_name);

        Ok(socket_path)
    }

    fn setup_tty_socket(&self, container_dir: &Path) -> Result<Option<RawFd>> {
        let tty_name = Self::generate_name(container_dir, TENANT_TTY);
        let csocketfd = if let Some(console_socket) = &self.base.console_socket {
            Some(tty::setup_console_socket(
                container_dir,
                console_socket,
                &tty_name,
            )?)
        } else {
            None
        };

        Ok(csocketfd)
    }

    fn generate_name(dir: &Path, prefix: &str) -> String {
        loop {
            let rand = fastrand::i32(..);
            let name = format!("{prefix}{rand:x}.sock");
            if !dir.join(&name).exists() {
                return name;
            }
        }
    }
}
