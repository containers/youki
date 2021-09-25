use anyhow::{bail, Context, Result};
use caps::Capability;
use nix::unistd;
use oci_spec::runtime::{
    Capabilities as SpecCapabilities, Capability as SpecCapability, LinuxBuilder,
    LinuxCapabilities, LinuxCapabilitiesBuilder, LinuxNamespace, LinuxNamespaceBuilder,
    LinuxNamespaceType, Process, ProcessBuilder, Spec, SpecBuilder,
};
use procfs::process::Namespace;

use std::{
    collections::HashMap,
    convert::TryFrom,
    fs,
    os::unix::prelude::RawFd,
    path::{Path, PathBuf},
    str::FromStr,
};

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

    /// Joins an existing container
    pub fn build(self) -> Result<()> {
        let container_dir = self.lookup_container_dir()?;
        let container = self.load_container_state(container_dir.clone())?;

        let spec = self.load_init_spec(&container_dir)?;
        let spec = self.adapt_spec_for_tenant(&spec, &container)?;

        log::debug!("{:#?}", spec);

        unistd::chdir(&*container_dir)?;
        let notify_path = Self::setup_notify_listener(&container_dir)?;
        // convert path of root file system of the container to absolute path
        let rootfs = fs::canonicalize(&spec.root().as_ref().context("no root in spec")?.path())?;

        // if socket file path is given in commandline options,
        // get file descriptors of console socket
        let csocketfd = self.setup_tty_socket(&container_dir)?;

        let use_systemd = self.should_use_systemd(&container);
        let rootless = Rootless::new(&spec)?;

        let mut builder_impl = ContainerBuilderImpl {
            init: false,
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
        };

        builder_impl.create()?;

        let mut notify_socket = NotifySocket::new(notify_path);
        notify_socket.notify_container_start()?;
        Ok(())
    }

    fn lookup_container_dir(&self) -> Result<PathBuf> {
        let container_dir = self.base.root_path.join(&self.base.container_id);
        if !container_dir.exists() {
            bail!("container {} does not exist", self.base.container_id);
        }

        Ok(container_dir)
    }

    fn load_init_spec(&self, container_dir: &Path) -> Result<Spec> {
        let spec_path = container_dir.join("config.json");

        let spec = Spec::load(spec_path).context("failed to load spec")?;
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

    fn adapt_spec_for_tenant(&self, spec: &Spec, container: &Container) -> Result<Spec> {
        let process = if let Some(ref process) = self.process {
            self.set_process(process)?
        } else {
            let mut process_builder = ProcessBuilder::default();

            process_builder = match self.set_working_dir()? {
                Some(cwd) => process_builder.cwd(cwd),
                None => process_builder,
            };

            process_builder = process_builder.args(self.set_args()?);
            process_builder = process_builder.env(self.set_environment()?);

            process_builder = match self.set_no_new_privileges() {
                Some(no_new_priv) => process_builder.no_new_privileges(no_new_priv),
                None => process_builder,
            };

            process_builder = match self.set_capabilities(spec)? {
                Some(caps) => process_builder.capabilities(caps),
                None => process_builder,
            };

            process_builder.build()?
        };

        if container.pid().is_none() {
            bail!("Could not retrieve container init pid");
        }

        let init_process = procfs::process::Process::new(container.pid().unwrap().as_raw())?;
        let ns = self.set_namespaces(init_process.namespaces()?)?;
        let linux = LinuxBuilder::default().namespaces(ns).build()?;

        let mut spec_builder = SpecBuilder::default()
            .process(process)
            .version(spec.version())
            .linux(linux);

        spec_builder = match spec.root() {
            Some(root) => spec_builder.root(root.clone()),
            None => spec_builder,
        };
        spec_builder = match spec.mounts() {
            Some(mounts) => spec_builder.mounts(mounts.clone()),
            None => spec_builder,
        };
        spec_builder = match spec.hostname() {
            Some(hostname) => spec_builder.hostname(hostname.clone()),
            None => spec_builder,
        };
        spec_builder = match spec.hooks() {
            Some(hooks) => spec_builder.hooks(hooks.clone()),
            None => spec_builder,
        };
        spec_builder = match spec.annotations() {
            Some(annotations) => spec_builder.annotations(annotations.clone()),
            None => spec_builder,
        };

        let spec = spec_builder.build()?;
        Ok(spec)
    }

    fn set_process(&self, process: &Path) -> Result<Process> {
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

    fn set_working_dir(&self) -> Result<Option<PathBuf>> {
        if let Some(ref cwd) = self.cwd {
            if cwd.is_relative() {
                bail!(
                    "Current working directory must be an absolute path, but is {}",
                    cwd.display()
                );
            }
            return Ok(Some(cwd.into()));
        }
        Ok(None)
    }

    fn set_args(&self) -> Result<Vec<String>> {
        if self.args.is_empty() {
            bail!("Container command was not specified")
        }

        Ok(self.args.clone())
    }

    fn set_environment(&self) -> Result<Vec<String>> {
        Ok(self
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect())
    }

    fn set_no_new_privileges(&self) -> Option<bool> {
        self.no_new_privs
    }

    fn set_capabilities(&self, spec: &Spec) -> Result<Option<LinuxCapabilities>> {
        if !self.capabilities.is_empty() {
            let mut caps: Vec<Capability> = Vec::with_capacity(self.capabilities.len());
            for cap in &self.capabilities {
                caps.push(Capability::from_str(cap)?);
            }

            let caps: SpecCapabilities =
                caps.iter().map(|c| SpecCapability::from_cap(*c)).collect();

            if let Some(ref spec_caps) = spec
                .process()
                .as_ref()
                .context("no process in spec")?
                .capabilities()
            {
                let mut cb = LinuxCapabilitiesBuilder::default();
                cb = match spec_caps.ambient() {
                    Some(ambient) => {
                        let ambient: SpecCapabilities = ambient.union(&caps).copied().collect();
                        cb.ambient(ambient)
                    }
                    None => cb,
                };
                cb = match spec_caps.bounding() {
                    Some(bounding) => {
                        let bounding: SpecCapabilities = bounding.union(&caps).copied().collect();
                        cb.bounding(bounding)
                    }
                    None => cb,
                };
                cb = match spec_caps.effective() {
                    Some(effective) => {
                        let effective: SpecCapabilities = effective.union(&caps).copied().collect();
                        cb.effective(effective)
                    }
                    None => cb,
                };
                cb = match spec_caps.inheritable() {
                    Some(inheritable) => {
                        let inheritable: SpecCapabilities =
                            inheritable.union(&caps).copied().collect();
                        cb.inheritable(inheritable)
                    }
                    None => cb,
                };
                cb = match spec_caps.permitted() {
                    Some(permitted) => {
                        let permitted: SpecCapabilities = permitted.union(&caps).copied().collect();
                        cb.permitted(permitted)
                    }
                    None => cb,
                };

                let c = cb.build()?;
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

    fn set_namespaces(&self, init_namespaces: Vec<Namespace>) -> Result<Vec<LinuxNamespace>> {
        let mut tenant_namespaces = Vec::with_capacity(init_namespaces.len());

        for ns_type in NAMESPACE_TYPES.iter().copied() {
            if let Some(init_ns) = init_namespaces.iter().find(|n| n.ns_type.eq(ns_type)) {
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
        let socket_path = container_dir.join(&notify_name);

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
            let name = format!("{}{:x}.sock", prefix, rand);
            if !dir.join(&name).exists() {
                return name;
            }
        }
    }
}
