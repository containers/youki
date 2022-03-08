use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    fs::{self},
    path::Component::RootDir,
};

use anyhow::{anyhow, bail, Context, Result};
use dbus::arg::RefArg;
use nix::{unistd::Pid, NixPath};
use std::path::{Path, PathBuf};

use super::{
    controller::Controller,
    controller_type::{ControllerType, CONTROLLER_TYPES},
    cpu::Cpu,
    cpuset::CpuSet,
    dbus::client::{Client, SystemdClient},
    memory::Memory,
    pids::Pids,
};
use crate::{
    common::{self, CgroupManager, ControllerOpt, FreezerState, PathBufExt},
    systemd::unified::Unified,
};
use crate::{stats::Stats, v2::manager::Manager as FsManager};

const CGROUP_CONTROLLERS: &str = "cgroup.controllers";
const CGROUP_SUBTREE_CONTROL: &str = "cgroup.subtree_control";

pub struct Manager {
    /// Root path of the cgroup hierarchy e.g. /sys/fs/cgroup
    root_path: PathBuf,
    /// Path relative to the root path e.g. /system.slice/youki-569d5ce3afe1074769f67.scope for rootfull containers
    /// and e.g. /user.slice/user-1000/user@1000.service/youki-569d5ce3afe1074769f67.scope for rootless containers
    cgroups_path: PathBuf,
    /// Combination of root path and cgroups path
    full_path: PathBuf,
    /// Destructured cgroups path as specified in the runtime spec e.g. system.slice:youki:569d5ce3afe1074769f67
    destructured_path: CgroupsPath,
    /// Name of the container e.g. 569d5ce3afe1074769f67
    container_name: String,
    /// Name of the systemd unit e.g. youki-569d5ce3afe1074769f67.scope
    unit_name: String,
    /// Client for communicating with systemd
    client: Client,
    /// Cgroup manager for the created transient unit
    fs_manager: FsManager,
    /// Last control group which is managed by systemd, e.g. /user.slice/user-1000/user@1000.service
    delegation_boundary: PathBuf,
}

/// Represents the systemd cgroups path:
/// It should be of the form [slice]:[scope_prefix]:[name].
/// The slice is the "parent" and should be expanded properly,
/// see expand_slice below.
#[derive(Debug)]
struct CgroupsPath {
    parent: String,
    prefix: String,
    name: String,
}

impl TryFrom<&Path> for CgroupsPath {
    type Error = anyhow::Error;

    fn try_from(cgroups_path: &Path) -> Result<Self, Self::Error> {
        // if cgroups_path was provided it should be of the form [slice]:[prefix]:[name],
        // for example: "system.slice:docker:1234".
        if cgroups_path.len() == 0 {
            bail!("no cgroups path has been provided");
        }

        let parts = cgroups_path
            .to_str()
            .ok_or_else(|| anyhow!("failed to parse cgroups path {:?}", cgroups_path))?
            .split(':')
            .collect::<Vec<&str>>();

        let destructured_path = match parts.len() {
            2 => CgroupsPath {
                parent: "".to_owned(),
                prefix: parts[0].to_owned(),
                name: parts[1].to_owned(),
            },
            3 => CgroupsPath {
                parent: parts[0].to_owned(),
                prefix: parts[1].to_owned(),
                name: parts[2].to_owned(),
            },
            _ => bail!("cgroup path {:?} is invalid", cgroups_path),
        };

        Ok(destructured_path)
    }
}

impl Display for CgroupsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.parent, self.prefix, self.name)
    }
}

/// ensures that a parent unit for the current unit is specified
fn ensure_parent_unit(cgroups_path: &mut CgroupsPath, use_system: bool) {
    if cgroups_path.parent.is_empty() {
        cgroups_path.parent = match use_system {
            true => "system.slice".to_owned(),
            false => "user.slice".to_owned(),
        }
    }
}

// custom debug impl as Manager contains fields that do not implement Debug
// and therefore Debug cannot be derived
impl Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manager")
            .field("root_path", &self.root_path)
            .field("cgroups_path", &self.cgroups_path)
            .field("full_path", &self.full_path)
            .field("destructured_path", &self.destructured_path)
            .field("container_name", &self.container_name)
            .field("unit_name", &self.unit_name)
            .finish()
    }
}

impl Manager {
    pub fn new(
        root_path: PathBuf,
        cgroups_path: PathBuf,
        container_name: String,
        use_system: bool,
    ) -> Result<Self> {
        let mut destructured_path = cgroups_path
            .as_path()
            .try_into()
            .with_context(|| format!("failed to destructure cgroups path {:?}", cgroups_path))?;
        ensure_parent_unit(&mut destructured_path, use_system);

        let client = match use_system {
            true => Client::new_system().context("failed to create system dbus client")?,
            false => Client::new_session().context("failed to create session dbus client")?,
        };

        let (cgroups_path, delegation_boundary) =
            Self::construct_cgroups_path(&destructured_path, &client)
                .context("failed to construct cgroups path")?;
        let full_path = root_path.join_safely(&cgroups_path)?;
        let fs_manager = FsManager::new(root_path.clone(), cgroups_path.clone())?;

        Ok(Manager {
            root_path,
            cgroups_path,
            full_path,
            container_name,
            unit_name: Self::get_unit_name(&destructured_path),
            destructured_path,
            client,
            fs_manager,
            delegation_boundary,
        })
    }

    /// get_unit_name returns the unit (scope) name from the path provided by the user
    /// for example: foo:docker:bar returns in '/docker-bar.scope'
    fn get_unit_name(cgroups_path: &CgroupsPath) -> String {
        // By default we create a scope unless specified explicitly.
        if !cgroups_path.name.ends_with(".slice") {
            return format!("{}-{}.scope", cgroups_path.prefix, cgroups_path.name);
        }
        cgroups_path.name.clone()
    }

    // get_cgroups_path generates a cgroups path from the one provided by the user via cgroupsPath.
    // an example of the final path: "/system.slice/youki-569d5ce3afe1074769f67.scope" or if we are
    // not running as root /user.slice/user-1000/user@1000.service/youki-569d5ce3afe1074769f67.scope
    fn construct_cgroups_path(
        cgroups_path: &CgroupsPath,
        client: &dyn SystemdClient,
    ) -> Result<(PathBuf, PathBuf)> {
        // if the user provided a '.slice' (as in a branch of a tree)
        // we need to convert it to a filesystem path.

        let parent = Self::expand_slice(&cgroups_path.parent)?;
        let systemd_root = client.control_cgroup_root()?;
        let unit_name = Self::get_unit_name(cgroups_path);

        let cgroups_path = systemd_root
            .join_safely(&parent)
            .with_context(|| format!("failed to join {:?} with {:?}", systemd_root, parent))?
            .join_safely(&unit_name)
            .with_context(|| format!("failed to join {:?} with {:?}", parent, unit_name))?;
        Ok((cgroups_path, systemd_root))
    }

    // systemd represents slice hierarchy using `-`, so we need to follow suit when
    // generating the path of slice. For example, 'test-a-b.slice' becomes
    // '/test.slice/test-a.slice/test-a-b.slice'.
    fn expand_slice(slice: &str) -> Result<PathBuf> {
        let suffix = ".slice";
        if slice.len() <= suffix.len() || !slice.ends_with(suffix) {
            bail!("invalid slice name: {}", slice);
        }
        if slice.contains('/') {
            bail!("invalid slice name: {}", slice);
        }
        let mut path = "".to_owned();
        let mut prefix = "".to_owned();
        let slice_name = slice.trim_end_matches(suffix);
        // if input was -.slice, we should just return root now
        if slice_name == "-" {
            return Ok(Path::new("/").to_path_buf());
        }
        for component in slice_name.split('-') {
            if component.is_empty() {
                bail!("invalid slice name: {}", slice);
            }
            // Append the component to the path and to the prefix.
            path = format!("{}/{}{}{}", path, prefix, component, suffix);
            prefix = format!("{}{}-", prefix, component);
        }
        Ok(Path::new(&path).to_path_buf())
    }

    /// ensures that each level in the downward path from the delegation boundary down to
    /// the scope or slice of the transient unit has all available controllers enabled
    fn ensure_controllers_attached(&self) -> Result<()> {
        let full_boundary_path = self.root_path.join_safely(&self.delegation_boundary)?;

        let controllers: Vec<String> = self
            .get_available_controllers(&full_boundary_path)?
            .into_iter()
            .map(|c| format!("{}{}", "+", c))
            .collect();

        Self::write_controllers(&full_boundary_path, &controllers)?;

        let mut current_path = full_boundary_path;
        let mut components = self
            .cgroups_path
            .strip_prefix(&self.delegation_boundary)?
            .components()
            .filter(|c| c.ne(&RootDir))
            .peekable();
        // Verify that *each level* in the downward path from the root cgroup
        // down to the cgroup_path provided by the user is a valid cgroup hierarchy.
        // containing the attached controllers.
        while let Some(component) = components.next() {
            current_path = current_path.join(component);
            if !current_path.exists() {
                log::warn!(
                    "{:?} does not exist. Resource restrictions might not work correctly",
                    current_path
                );
                return Ok(());
            }

            // last component cannot have subtree_control enabled due to internal process constraint
            // if this were set, writing to the cgroups.procs file will fail with Erno 16 (device or resource busy)
            if components.peek().is_some() {
                Self::write_controllers(&current_path, &controllers)?;
            }
        }

        Ok(())
    }

    fn get_available_controllers<P: AsRef<Path>>(
        &self,
        cgroups_path: P,
    ) -> Result<Vec<ControllerType>> {
        let controllers_path = self.root_path.join(cgroups_path).join(CGROUP_CONTROLLERS);
        if !controllers_path.exists() {
            bail!(
                "cannot get available controllers. {:?} does not exist",
                controllers_path
            )
        }

        let mut controllers = Vec::new();
        for controller in fs::read_to_string(&controllers_path)?.split_whitespace() {
            match controller {
                "cpu" => controllers.push(ControllerType::Cpu),
                "memory" => controllers.push(ControllerType::Memory),
                "pids" => controllers.push(ControllerType::Pids),
                _ => continue,
            }
        }

        Ok(controllers)
    }

    fn write_controllers(path: &Path, controllers: &[String]) -> Result<()> {
        for controller in controllers {
            common::write_cgroup_file_str(path.join(CGROUP_SUBTREE_CONTROL), controller)?;
        }

        Ok(())
    }
}

impl CgroupManager for Manager {
    fn add_task(&self, pid: Pid) -> Result<()> {
        // Dont attach any pid to the cgroup if -1 is specified as a pid
        if pid.as_raw() == -1 {
            return Ok(());
        }

        log::debug!("Starting {:?}", self.unit_name);
        self.client
            .start_transient_unit(
                &self.container_name,
                pid.as_raw() as u32,
                &self.destructured_path.parent,
                &self.unit_name,
            )
            .with_context(|| {
                format!(
                    "failed to create unit {} for container {}",
                    self.unit_name, self.container_name
                )
            })?;

        Ok(())
    }

    fn apply(&self, controller_opt: &ControllerOpt) -> Result<()> {
        let mut properties: HashMap<&str, Box<dyn RefArg>> = HashMap::new();
        let systemd_version = self
            .client
            .systemd_version()
            .context("could not retrieve systemd version")?;

        for controller in CONTROLLER_TYPES {
            match controller {
                ControllerType::Cpu => {
                    Cpu::apply(controller_opt, systemd_version, &mut properties)?
                }

                ControllerType::CpuSet => {
                    CpuSet::apply(controller_opt, systemd_version, &mut properties)?
                }

                ControllerType::Pids => {
                    Pids::apply(controller_opt, systemd_version, &mut properties)?
                }
                ControllerType::Memory => {
                    Memory::apply(controller_opt, systemd_version, &mut properties)?
                }
                _ => {}
            };
        }

        Unified::apply(controller_opt, systemd_version, &mut properties)?;
        log::debug!("{:?}", properties);

        if !properties.is_empty() {
            self.ensure_controllers_attached()
                .context("failed to attach controllers")?;

            self.client
                .set_unit_properties(&self.unit_name, &properties)
                .context("could not apply resource restrictions")?;
        }

        Ok(())
    }

    fn remove(&self) -> Result<()> {
        log::debug!("remove {}", self.unit_name);
        if self.client.transient_unit_exists(&self.unit_name) {
            self.client
                .stop_transient_unit(&self.unit_name)
                .with_context(|| {
                    format!("could not remove control group {}", self.destructured_path)
                })?;
        }

        Ok(())
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        self.fs_manager.freeze(state)
    }

    fn stats(&self) -> Result<Stats> {
        self.fs_manager.stats()
    }

    fn get_all_pids(&self) -> Result<Vec<Pid>> {
        common::get_all_pids(&self.full_path)
    }
}

#[cfg(test)]
mod tests {
    use crate::systemd::dbus::client::SystemdClient;

    use super::*;

    struct TestSystemdClient {}

    impl SystemdClient for TestSystemdClient {
        fn is_system(&self) -> bool {
            true
        }

        fn transient_unit_exists(&self, _: &str) -> bool {
            true
        }

        fn start_transient_unit(
            &self,
            _container_name: &str,
            _pid: u32,
            _parent: &str,
            _unit_name: &str,
        ) -> Result<()> {
            Ok(())
        }

        fn stop_transient_unit(&self, _unit_name: &str) -> Result<()> {
            Ok(())
        }

        fn set_unit_properties(
            &self,
            _unit_name: &str,
            _properties: &HashMap<&str, Box<dyn RefArg>>,
        ) -> Result<()> {
            Ok(())
        }

        fn systemd_version(&self) -> Result<u32> {
            Ok(245)
        }

        fn control_cgroup_root(&self) -> Result<PathBuf> {
            Ok(PathBuf::from("/"))
        }
    }

    #[test]
    fn expand_slice_works() -> Result<()> {
        assert_eq!(
            Manager::expand_slice("test-a-b.slice")?,
            PathBuf::from("/test.slice/test-a.slice/test-a-b.slice"),
        );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_a_complex_slice() -> Result<()> {
        let cgroups_path = Path::new("test-a-b.slice:docker:foo")
            .try_into()
            .context("construct path")?;

        assert_eq!(
            Manager::construct_cgroups_path(&cgroups_path, &TestSystemdClient {})?.0,
            PathBuf::from("/test.slice/test-a.slice/test-a-b.slice/docker-foo.scope"),
        );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_a_simple_slice() -> Result<()> {
        let cgroups_path = Path::new("machine.slice:libpod:foo")
            .try_into()
            .context("construct path")?;

        assert_eq!(
            Manager::construct_cgroups_path(&cgroups_path, &TestSystemdClient {})?.0,
            PathBuf::from("/machine.slice/libpod-foo.scope"),
        );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_without_parent() -> Result<()> {
        let mut cgroups_path = Path::new(":docker:foo")
            .try_into()
            .context("construct path")?;
        ensure_parent_unit(&mut cgroups_path, true);

        assert_eq!(
            Manager::construct_cgroups_path(&cgroups_path, &TestSystemdClient {})?.0,
            PathBuf::from("/system.slice/docker-foo.scope"),
        );

        Ok(())
    }
}
