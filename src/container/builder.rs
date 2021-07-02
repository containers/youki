#![allow(unused_imports, unused_variables)]

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use nix::unistd;
use oci_spec::Spec;

use crate::{command::{Syscall, linux::{self, LinuxCommand}, syscall::create_syscall}, notify_socket::NotifyListener, rootless::{self, lookup_map_binaries, should_use_rootless, Rootless}, tty, utils};

use super::{builder_impl::ContainerBuilderImpl, Container, ContainerStatus};

pub struct ContainerBuilder {
    // defaults
    ///
    init: bool,
    ///
    use_systemd: bool,
    ///
    syscall: LinuxCommand,
    ////
    root_path: PathBuf,

    // required
    ///
    container_id: String,
    ///     
    bundle: Option<PathBuf>,

    // optional
    ///
    pid_file: Option<PathBuf>,
    ///
    console_socket: Option<PathBuf>,
}

impl ContainerBuilder {
    pub fn new_init<P: Into<PathBuf>>(container_id: String, bundle: P) -> Result<Self> {
        let bundle = Some(fs::canonicalize(bundle.into())?);
        let root_path = PathBuf::from("/run/youki");

        Ok(Self {
            init: true,
            use_systemd: true,
            syscall: LinuxCommand,
            root_path,
            container_id,
            bundle,
            pid_file: None,
            console_socket: None,
        })
    }

    pub fn new_tenant(container_id: String) -> Self {
        let root_path = PathBuf::from("/run/youki");

        Self {
            init: false,
            use_systemd: true,
            syscall: LinuxCommand,
            root_path,
            container_id,
            bundle: None,
            pid_file: None,
            console_socket: None,
        }
    }

    pub fn with_systemd(mut self, should_use: bool) -> Self {
        self.use_systemd = should_use;
        self
    }

    pub fn with_root_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.root_path = path.into();
        self
    }

    pub fn with_pid_file<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.pid_file = Some(path.into());
        self
    }

    pub fn with_console_socket<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.console_socket = Some(path.into());
        self
    }

    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        todo!();
    }

    pub fn with_cwd<P: Into<PathBuf>>(mut self, path: P) -> Self {
        todo!();
    }

    pub fn with_container_command(mut self, command: Vec<String>) -> Self {
        todo!();
    }

    pub fn build(mut self) -> Result<()> {
        let container_dir = self.prepare_container_dir()?;
        let spec = self.load_and_safeguard_spec(&container_dir)?;
        unistd::chdir(&*container_dir)?;

        let container = if self.init {
            Some(self.create_container_state(&container_dir)?)
        } else {
            None
        };

        let notify_socket: NotifyListener = NotifyListener::new(&container_dir)?;
        // convert path of root file system of the container to absolute path
        let rootfs = fs::canonicalize(&spec.root.path)?;

        // if socket file path is given in commandline options,
        // get file descriptors of console socket
        let csocketfd = if let Some(console_socket) = &self.console_socket {
            Some(tty::setup_console_socket(&container_dir, console_socket)?)
        } else {
            None
        };

        let rootless = self.is_rootless_required(&spec)?;

        let mut builder_impl = ContainerBuilderImpl {
            init: self.init,
            use_systemd: self.use_systemd,
            root_path: self.root_path,
            container_id: self.container_id,
            pid_file: self.pid_file,
            syscall: self.syscall,
            console_socket: csocketfd,
            rootless,
            container_dir,
            spec,
            rootfs,
            notify_socket,
            container,
        };

        builder_impl.create()?;
        Ok(())
    }

    fn prepare_container_dir(&mut self) -> Result<PathBuf> {
        let container_dir = self.root_path.join(&self.container_id);
        log::debug!("container directory will be {:?}", container_dir);

        match (self.init, container_dir.exists()) {
            (true, true) => bail!("container {} already exists", self.container_id),
            (true, false) => utils::create_dir_all(&container_dir)?,
            (false, true) => {}
            (false, false) => bail!("container {} does not exist", self.container_id),
        }

        Ok(container_dir)
    }

    fn load_and_safeguard_spec(&self, container_dir: &Path) -> Result<Spec> {
        let spec_path = if self.init {
            let config_path = self.bundle.as_ref().unwrap().join("config.json");
            fs::copy(&config_path, container_dir.join("config.json"))?;
            config_path
        } else {
            container_dir.join("config.json")
        };

        let spec = oci_spec::Spec::load(spec_path)?;
        Ok(spec)
    }

    fn is_rootless_required(&self, spec: &Spec) -> Result<Option<Rootless>> {
        let linux = spec.linux.as_ref().unwrap();

        let rootless = if should_use_rootless() {
            log::debug!("rootless container should be created");
            log::warn!(
                "resource constraints and multi id mapping is unimplemented for rootless containers"
            );
            rootless::validate(spec)?;
            let mut rootless = Rootless::from(linux);
            if let Some((uid_binary, gid_binary)) = lookup_map_binaries(linux)? {
                rootless.newuidmap = Some(uid_binary);
                rootless.newgidmap = Some(gid_binary);
            }
            Some(rootless)
        } else {
            None
        };

        Ok(rootless)
    }

    fn create_container_state(&self, container_dir: &Path) -> Result<Container> {
        let container = Container::new(
            &self.container_id,
            ContainerStatus::Creating,
            None,
            self.bundle.as_ref().unwrap().to_str().unwrap(),
            &container_dir,
        )?;
        container.save()?;
        Ok(container)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // required values (must be specified in new...)
    // - create
    //   - id
    //   - bundle
    // - exec
    //   - id
    //
    // use with_... methods to specify
    // optional values
    // - console-socket
    // - pid-file
    //
    // overwritable values
    // - systemd (default true)
    // - root_path (default /run/youki)
    //
    // overwritable values (for exec only?)
    // - env
    // - cwd
    // - container command
    //
    // calculated in build()
    // computed values
    // - rootless
    // - container_dir
    // - spec
    // - notify_socket
    // - container

    // create
    fn test_create_init() -> Result<()> {
        let id = "".to_owned();
        let bundle = PathBuf::from("");
        let pid_file = PathBuf::from("");
        let console_socket = PathBuf::from("");
        let root_path = PathBuf::from("");

        let container = ContainerBuilder::new_init(id, bundle)?
            .with_pid_file(pid_file) // optional
            .with_console_socket(console_socket) //optional
            .with_systemd(false) // overwrite default
            .with_root_path(root_path) // overwrite default
            .build()?;

        Ok(())
    }

    // exec
    fn test_create_tenant() -> Result<()> {
        let id = "".to_owned();
        let pid_file = PathBuf::from("");
        let console_socket = PathBuf::from("");
        let cwd = PathBuf::from("");
        let env = HashMap::new();

        let container = ContainerBuilder::new_tenant(id)
            .with_pid_file(pid_file)
            .with_console_socket(console_socket)
            .with_cwd(cwd)
            .with_env(env)
            .with_container_command(vec!["sleep".to_owned(), "9001".to_owned()])
            .build()?;

        Ok(())
    }
}
