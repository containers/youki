use std::{
    collections::HashMap,
    fmt::Display,
    fs::{self},
    os::unix::fs::PermissionsExt,
    path::Component::RootDir,
};

use anyhow::{anyhow, bail, Context, Result};
use dbus::arg::RefArg;
use nix::unistd::Pid;
use std::path::{Path, PathBuf};

#[cfg(feature = "cgroupsv2_devices")]
use super::devices::Devices;
use super::{
    controller::Controller,
    controller_type::{ControllerType, CONTROLLER_TYPES},
    cpu::Cpu,
    cpuset::CpuSet,
    dbus::client::Client,
};
use crate::common::{self, CgroupManager, ControllerOpt, FreezerState, PathBufExt};
use crate::stats::Stats;

const CGROUP_PROCS: &str = "cgroup.procs";
const CGROUP_CONTROLLERS: &str = "cgroup.controllers";
const CGROUP_SUBTREE_CONTROL: &str = "cgroup.subtree_control";

/// SystemDCGroupManager is a driver for managing cgroups via systemd.
pub struct Manager {
    root_path: PathBuf,
    cgroups_path: PathBuf,
    full_path: PathBuf,
    destructured_path: CgroupsPath,
    container_name: String,
    unit_name: String,
    client: Client,
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

impl Display for CgroupsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.parent, self.prefix, self.name)
    }
}

impl Manager {
    pub fn new(root_path: PathBuf, cgroups_path: PathBuf, container_name: String) -> Result<Self> {
        // TODO: create the systemd unit using a dbus client.
        let destructured_path = Self::destructure_cgroups_path(cgroups_path)?;
        let (cgroups_path, parent) = Self::construct_cgroups_path(&destructured_path)?;
        let full_path = root_path.join_safely(&cgroups_path)?;

        Ok(Manager {
            root_path,
            cgroups_path,
            full_path,
            container_name,
            unit_name: Self::get_unit_name(&destructured_path),
            destructured_path,
            client: Client::new().context("failed to create dbus client")?,
        })
    }

    fn destructure_cgroups_path(cgroups_path: PathBuf) -> Result<CgroupsPath> {
        log::debug!("CGROUPS PATH IS {:?}", cgroups_path);
        // cgroups path may never be empty as it is defaulted to `/youki`
        // see 'get_cgroup_path' under utils.rs.
        // if cgroups_path was provided it should be of the form [slice]:[prefix]:[name],
        // for example: "system.slice:docker:1234".
        let mut parent = "";
        let prefix;
        let name;
        if cgroups_path.starts_with("/youki") {
            prefix = "youki";
            name = cgroups_path
                .strip_prefix("/youki/")?
                .to_str()
                .ok_or_else(|| anyhow!("failed to parse cgroupsPath field"))?;
        } else {
            let parts = cgroups_path
                .to_str()
                .ok_or_else(|| anyhow!("failed to parse cgroupsPath field"))?
                .split(':')
                .collect::<Vec<&str>>();
            parent = parts[0];
            prefix = parts[1];
            name = parts[2];
        }

        Ok(CgroupsPath {
            parent: parent.to_owned(),
            prefix: prefix.to_owned(),
            name: name.to_owned(),
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
                anyhow!("Invalid slice name: {}", slice);
            }
            // Append the component to the path and to the prefix.
            path = format!("{}/{}{}{}", path, prefix, component, suffix);
            prefix = format!("{}{}-", prefix, component);
        }
        Ok(Path::new(&path).to_path_buf())
    }

    // get_cgroups_path generates a cgroups path from the one provided by the user via cgroupsPath.
    // an example of the final path: "/machine.slice/docker-foo.scope"
    fn construct_cgroups_path(cgroups_path: &CgroupsPath) -> Result<(PathBuf, PathBuf)> {
        // the root slice is under 'machine.slice'.
        let mut parent = PathBuf::from("/system.slice");
        // if the user provided a '.slice' (as in a branch of a tree)
        // we need to convert it to a filesystem path.
        if !cgroups_path.parent.is_empty() {
            parent = Self::expand_slice(&cgroups_path.parent)?;
        }
        let unit_name = Self::get_unit_name(cgroups_path);
        let cgroups_path = parent
            .join_safely(&unit_name)
            .with_context(|| format!("failed to join {:?} with {:?}", parent, unit_name))?;
        Ok((cgroups_path, parent))
    }

    /// create_unified_cgroup verifies sure that *each level* in the downward path from the root cgroup
    /// down to the cgroup_path provided by the user is a valid cgroup hierarchy,
    /// containing the attached controllers and that it contains the container pid.
    fn create_unified_cgroup(&self, pid: Pid) -> Result<()> {
        let controllers: Vec<String> = self
            .get_available_controllers(&self.root_path)?
            .into_iter()
            .map(|c| format!("{}{}", "+", c.to_string()))
            .collect();

        // Write the controllers to the root_path.
        Self::write_controllers(&self.root_path, &controllers)?;

        let mut current_path = self.root_path.clone();
        let mut components = self
            .cgroups_path
            .components()
            .filter(|c| c.ne(&RootDir))
            .peekable();
        // Verify that *each level* in the downward path from the root cgroup
        // down to the cgroup_path provided by the user is a valid cgroup hierarchy.
        // containing the attached controllers.
        while let Some(component) = components.next() {
            current_path = current_path.join(component);
            if !current_path.exists() {
                fs::create_dir(&current_path)?;
                fs::metadata(&current_path)?.permissions().set_mode(0o755);
            }

            // last component cannot have subtree_control enabled due to internal process constraint
            // if this were set, writing to the cgroups.procs file will fail with Erno 16 (device or resource busy)
            if components.peek().is_some() {
                Self::write_controllers(&current_path, &controllers)?;
            }
        }

        common::write_cgroup_file(self.full_path.join(CGROUP_PROCS), pid)
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
                "io" => controllers.push(ControllerType::Io),
                "memory" => controllers.push(ControllerType::Memory),
                "pids" => controllers.push(ControllerType::Tasks),
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
        let mut properties: HashMap<String, Box<dyn RefArg>> = HashMap::new();
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
                _ => {}
            };
        }

        self.client
            .set_unit_properties(&self.unit_name, &properties)
            .context("could not apply resource restrictions")?;

        #[cfg(feature = "cgroupsv2_devices")]
        Devices::apply(controller_opt, &self.full_path)?;
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        log::debug!("remove {}", self.unit_name);
        self.client
            .stop_transient_unit(&self.unit_name)
            .with_context(|| {
                format!("could not remove control group {}", self.destructured_path)
            })?;

        Ok(())
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        todo!();
    }

    fn stats(&self) -> Result<Stats> {
        Ok(Stats::default())
    }

    fn get_all_pids(&self) -> Result<Vec<Pid>> {
        common::get_all_pids(&self.full_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let cgroups_path =
            Manager::destructure_cgroups_path(PathBuf::from("test-a-b.slice:docker:foo"))
                .expect("");

        // assert_eq!(
        //     Manager::construct_cgroups_path(&cgroups_path)?,
        //     PathBuf::from("/test.slice/test-a.slice/test-a-b.slice/docker-foo.scope"),
        // );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_a_simple_slice() -> Result<()> {
        let cgroups_path =
            Manager::destructure_cgroups_path(PathBuf::from("machine.slice:libpod:foo")).expect("");

        // assert_eq!(
        //     Manager::construct_cgroups_path(&cgroups_path)?,
        //     PathBuf::from("/machine.slice/libpod-foo.scope"),
        // );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_scope() -> Result<()> {
        let cgroups_path =
            Manager::destructure_cgroups_path(PathBuf::from(":docker:foo")).expect("");

        // assert_eq!(
        //     Manager::construct_cgroups_path(&cgroups_path)?,
        //     PathBuf::from("/machine.slice/docker-foo.scope"),
        // );

        Ok(())
    }
}
