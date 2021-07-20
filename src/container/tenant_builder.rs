use anyhow::{bail, Context, Result};
use caps::Capability;
use nix::unistd;
use oci_spec::{
    LinuxCapabilities, LinuxNamespace, LinuxNamespaceType, Process, Spec,
};

use std::{
    collections::HashMap,
    convert::TryFrom,
    ffi::{CString, OsString},
    fs,
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    notify_socket::NotifySocket, rootless::detect_rootless, stdio::FileDescriptor, tty, utils,
};

use super::{builder::ContainerBuilder, builder_impl::ContainerBuilderImpl, Container};

const NAMESPACE_TYPES: &[&str] = &["ipc", "uts", "net", "pid", "mnt", "cgroup"];
const TENANT_NOTIFY: &str = "tenant-notify-";
const TENANT_TTY: &str = "tenant-tty-";

/// Builder that can be used to configure the properties of a process
/// that will join an existing container sandbox
pub struct TenantContainerBuilder {
    base: ContainerBuilder,
    env: HashMap<String, String>,
    cwd: Option<PathBuf>,
    args: Vec<String>,
    no_new_privs: Option<bool>,
    capabilities: Vec<String>,
    process: Option<PathBuf>,
}

impl TenantContainerBuilder {
    /// Generates the base configuration for a process that will join
    /// an existing container sandbox from which configuration methods
    /// can be chained
    pub(super) fn new(builder: ContainerBuilder) -> Self {
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
    pub fn with_cwd<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.cwd = Some(path.into());
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

    pub fn with_process<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.process = Some(path.into());
        self
    }

    /// Joins an existing container
    pub fn build(self) -> Result<()> {
        let container_dir = self.lookup_container_dir()?;
        let container = self.load_container_state(container_dir.clone())?;
        let mut spec = self.load_init_spec(&container_dir)?;
        self.adapt_spec_for_tenant(&mut spec, &container)?;
        log::debug!("{:#?}", spec);

        unistd::chdir(&*container_dir)?;
        let notify_path = Self::setup_notify_listener(&container_dir)?;
        // convert path of root file system of the container to absolute path
        let rootfs = fs::canonicalize(&spec.root.path)?;

        // if socket file path is given in commandline options,
        // get file descriptors of console socket
        let csocketfd = self.setup_tty_socket(&container_dir)?;

        let use_systemd = self.should_use_systemd(&container);
        let rootless = detect_rootless(&spec)?;

        let mut builder_impl = ContainerBuilderImpl {
            init: false,
            syscall: self.base.syscall,
            container_id: self.base.container_id,
            pid_file: self.base.pid_file,
            console_socket: csocketfd,
            use_systemd,
            container_dir,
            spec,
            rootfs,
            rootless,
            notify_path: notify_path.clone(),
            container: None,
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

        let spec = oci_spec::Spec::load(spec_path).context("failed to load spec")?;
        Ok(spec)
    }

    fn load_container_state(&self, container_dir: PathBuf) -> Result<Container> {
        let container = Container::load(container_dir)?.refresh_status()?;
        if !container.can_exec() {
            bail!(
                "Cannot exec as container is in state {}",
                container.status()
            );
        }

        Ok(container)
    }

    fn adapt_spec_for_tenant(&self, spec: &mut Spec, container: &Container) -> Result<()> {
        if let Some(ref process) = self.process {
            self.set_process(spec, process)?;
        } else {
            self.set_working_dir(spec)?;
            self.set_args(spec)?;
            self.set_environment(spec)?;
            self.set_no_new_privileges(spec);
            self.set_capabilities(spec)?;
        }

        if container.pid().is_none() {
            bail!("Could not retrieve container init pid");
        }

        let init_process = procfs::process::Process::new(container.pid().unwrap().as_raw())?;
        self.set_namespaces(spec, init_process.namespaces()?)?;

        Ok(())
    }

    fn set_process(&self, spec: &mut Spec, process: &Path) -> Result<()> {
        if !process.exists() {
            bail!(
                "Process.json file does not exist at specified path {}",
                process.display()
            )
        }

        let process = utils::open(process)?;
        let process_spec: Process = serde_json::from_reader(process)?;
        spec.process = process_spec;
        Ok(())
    }

    fn set_working_dir(&self, spec: &mut Spec) -> Result<()> {
        if let Some(ref cwd) = self.cwd {
            if cwd.is_relative() {
                bail!(
                    "Current working directory must be an absolute path, but is {}",
                    cwd.display()
                );
            }

            spec.process.cwd = cwd.to_string_lossy().to_string();
        }

        Ok(())
    }

    fn set_args(&self, spec: &mut Spec) -> Result<()> {
        if self.args.is_empty() {
            bail!("Container command was not specified")
        }

        spec.process.args = self.args.clone();
        Ok(())
    }

    fn set_environment(&self, spec: &mut Spec) -> Result<()> {
        spec.process.env.append(
            &mut self
                .env
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect(),
        );

        Ok(())
    }

    fn set_no_new_privileges(&self, spec: &mut Spec) {
        if let Some(no_new_privs) = self.no_new_privs {
            spec.process.no_new_privileges = no_new_privs;
        }
    }

    fn set_capabilities(&self, spec: &mut Spec) -> Result<()> {
        if !self.capabilities.is_empty() {
            let mut caps: Vec<Capability> = Vec::with_capacity(self.capabilities.len());
            for cap in &self.capabilities {
                caps.push(Capability::from_str(cap)?);
            }

            if let Some(ref mut spec_caps) = spec.process.capabilities {
                spec_caps.ambient.append(&mut caps.clone());
                spec_caps.bounding.append(&mut caps.clone());
                spec_caps.effective.append(&mut caps.clone());
                spec_caps.inheritable.append(&mut caps.clone());
                spec_caps.permitted.append(&mut caps);
            } else {
                spec.process.capabilities = Some(LinuxCapabilities {
                    ambient: caps.clone(),
                    bounding: caps.clone(),
                    effective: caps.clone(),
                    inheritable: caps.clone(),
                    permitted: caps,
                })
            }
        }

        Ok(())
    }

    fn set_namespaces(&self, spec: &mut Spec, init_namespaces: Vec<Namespace>) -> Result<()> {
        let mut tenant_namespaces = Vec::with_capacity(init_namespaces.len());

        for ns_type in NAMESPACE_TYPES.iter().copied() {
            if let Some(init_ns) = init_namespaces.iter().find(|n| n.ns_type.eq(ns_type)) {
                let tenant_ns = LinuxNamespaceType::try_from(ns_type)?;
                tenant_namespaces.push(LinuxNamespace {
                    typ: tenant_ns,
                    path: Some(init_ns.path.to_string_lossy().to_string()),
                })
            }
        }

        let mut linux = spec.linux.as_mut().unwrap();
        linux.namespaces = tenant_namespaces;
        Ok(())
    }

    fn should_use_systemd(&self, container: &Container) -> bool {
        if let Some(use_systemd) = container.systemd() {
            return use_systemd;
        }

        false
    }

    fn setup_notify_listener(container_dir: &Path) -> Result<PathBuf> {
        let notify_name = Self::generate_name(&container_dir, TENANT_NOTIFY);
        let socket_path = container_dir.join(&notify_name);

        Ok(socket_path)
    }

    fn setup_tty_socket(&self, container_dir: &Path) -> Result<Option<FileDescriptor>> {
        let tty_name = Self::generate_name(&container_dir, TENANT_TTY);
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

// Can be removed once https://github.com/eminence/procfs/pull/135 is available
trait GetNamespace {
    fn namespaces(&self) -> Result<Vec<Namespace>>;
}

impl GetNamespace for procfs::process::Process {
    /// Describes namespaces to which the process with the corresponding PID belongs.
    /// Doc reference: https://man7.org/linux/man-pages/man7/namespaces.7.html
    fn namespaces(&self) -> Result<Vec<Namespace>> {
        let proc_path = PathBuf::from(format!("/proc/{}", self.pid()));
        let ns = proc_path.join("ns");
        let mut namespaces = Vec::new();
        for entry in fs::read_dir(ns)? {
            let entry = entry?;
            let path = entry.path();
            let ns_type = entry.file_name();
            let cstr = CString::new(path.as_os_str().as_bytes()).unwrap();

            let mut stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::stat(cstr.as_ptr(), &mut stat) } != 0 {
                bail!("Unable to stat {:?}", path);
            }

            namespaces.push(Namespace {
                ns_type,
                path,
                identifier: stat.st_ino,
                device_id: stat.st_dev,
            })
        }

        Ok(namespaces)
    }
}

/// Information about a namespace
///
/// See also the [Process::namespaces()] method
#[derive(Debug, Clone)]
pub struct Namespace {
    /// Namespace type
    pub ns_type: OsString,
    /// Handle to the namespace
    pub path: PathBuf,
    /// Namespace identifier (inode number)
    pub identifier: u64,
    /// Device id of the namespace
    pub device_id: u64,
}

impl PartialEq for Namespace {
    fn eq(&self, other: &Self) -> bool {
        // see https://lore.kernel.org/lkml/87poky5ca9.fsf@xmission.com/
        self.identifier == other.identifier && self.device_id == other.device_id
    }
}

impl Eq for Namespace {}
