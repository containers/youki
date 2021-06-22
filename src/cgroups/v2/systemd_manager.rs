use std::{
    fs::{self},
    os::unix::fs::PermissionsExt,
};

use anyhow::{anyhow, bail, Result};
use nix::unistd::Pid;
use oci_spec::LinuxResources;
use std::path::{Path, PathBuf};

use super::{cpu::Cpu, cpuset::CpuSet, hugetlb::HugeTlb, io::Io, memory::Memory, pids::Pids};
use crate::cgroups::common;
use crate::cgroups::common::CgroupManager;
use crate::cgroups::v2::controller::Controller;
use crate::cgroups::v2::controller_type::ControllerType;
use crate::utils::PathBufExt;

const CGROUP_PROCS: &str = "cgroup.procs";
const CGROUP_CONTROLLERS: &str = "cgroup.controllers";
const CGROUP_SUBTREE_CONTROL: &str = "cgroup.subtree_control";

// v2 systemd only supports cpu, io, memory and pids.
const CONTROLLER_TYPES: &[ControllerType] = &[
    ControllerType::Cpu,
    ControllerType::Io,
    ControllerType::Memory,
    ControllerType::Pids,
];

/// SystemDCGroupManager is a driver for managing cgroups via systemd.
pub struct SystemDCGroupManager {
    root_path: PathBuf,
    cgroups_path: CgroupsPath,
}

/// Represents the systemd cgroups path:
/// It should be of the form [slice]:[scope_prefix]:[name].
/// The slice is the "parent" and should be expanded properly,
/// see expand_slice below.
struct CgroupsPath {
    parent: String,
    scope: String,
    name: String,
}

impl SystemDCGroupManager {
    pub fn new(root_path: PathBuf, cgroups_path: PathBuf) -> Result<Self> {
        // cgroups path may never be empty as it is defaulted to `/youki`
        // see 'get_cgroup_path' under utils.rs.
        // if cgroups_path was provided it should be of the form [slice]:[scope_prefix]:[name],
        // for example: "system.slice:docker:1234".
        let mut parent = "";
        let scope;
        let name;
        if cgroups_path.starts_with("/youki") {
            scope = "youki";
            name = cgroups_path
                .strip_prefix("/youki/")?
                .to_str()
                .ok_or_else(|| anyhow!("Failed to parse cgroupsPath field."))?;
        } else {
            let parts = cgroups_path
                .to_str()
                .ok_or_else(|| anyhow!("Failed to parse cgroupsPath field."))?
                .split(':')
                .collect::<Vec<&str>>();
            parent = parts[0];
            scope = parts[1];
            name = parts[2];
        }

        // TODO: create the systemd unit using a dbus client.

        Ok(SystemDCGroupManager {
            root_path,
            cgroups_path: CgroupsPath {
                parent: parent.to_owned(),
                scope: scope.to_owned(),
                name: name.to_owned(),
            },
        })
    }

    /// get_unit_name returns the unit (scope) name from the path provided by the user
    /// for example: foo:docker:bar returns in '/docker-bar.scope'
    fn get_unit_name(&self) -> String {
        // By default we create a scope unless specified explicitly.
        if !self.cgroups_path.name.ends_with(".slice") {
            return format!(
                "{}-{}.scope",
                self.cgroups_path.scope, self.cgroups_path.name
            );
        }
        self.cgroups_path.name.clone()
    }

    // systemd represents slice hierarchy using `-`, so we need to follow suit when
    // generating the path of slice. For example, 'test-a-b.slice' becomes
    // '/test.slice/test-a.slice/test-a-b.slice'.
    fn expand_slice(&self, slice: &str) -> Result<PathBuf> {
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
    fn get_cgroups_path(&self) -> Result<PathBuf> {
        // the root slice is under 'machine.slice'.
        let mut slice = Path::new("/machine.slice").to_path_buf();
        // if the user provided a '.slice' (as in a branch of a tree)
        // we need to "unpack it".
        if !self.cgroups_path.parent.is_empty() {
            slice = self.expand_slice(&self.cgroups_path.parent)?;
        }
        let unit_name = self.get_unit_name();
        let cgroups_path = slice.join(unit_name);
        Ok(cgroups_path)
    }

    /// create_unified_cgroup verifies sure that *each level* in the downward path from the root cgroup
    /// down to the cgroup_path provided by the user is a valid cgroup hierarchy,
    /// containing the attached controllers and that it contains the container pid.
    fn create_unified_cgroup(&self, pid: Pid) -> Result<PathBuf> {
        let cgroups_path = self.get_cgroups_path()?;
        let full_path = self.root_path.join_absolute_path(&cgroups_path)?;
        let controllers: Vec<String> = self
            .get_available_controllers(&self.root_path)?
            .into_iter()
            .map(|c| format!("{}{}", "+", c.to_string()))
            .collect();

        // Write the controllers to the root_path.
        Self::write_controllers(&self.root_path, &controllers)?;

        let mut current_path = self.root_path.clone();
        let mut components = cgroups_path.components().skip(1).peekable();
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

        common::write_cgroup_file(full_path.join(CGROUP_PROCS), &pid)?;
        Ok(full_path)
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
                "pids" => controllers.push(ControllerType::Pids),
                _ => continue,
            }
        }

        Ok(controllers)
    }

    fn write_controllers(path: &Path, controllers: &Vec<String>) -> Result<()> {
        for controller in controllers {
            common::write_cgroup_file_str(path.join(CGROUP_SUBTREE_CONTROL), controller)?;
        }

        Ok(())
    }
}

impl CgroupManager for SystemDCGroupManager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid) -> Result<()> {
        // Dont attach any pid to the cgroup if -1 is specified as a pid
        if pid.as_raw() == -1 {
            return Ok(());
        }
        let full_cgroup_path = self.create_unified_cgroup(pid)?;

        for controller in CONTROLLER_TYPES {
            match controller {
                ControllerType::Cpu => Cpu::apply(linux_resources, &full_cgroup_path)?,
                ControllerType::CpuSet => CpuSet::apply(linux_resources, &full_cgroup_path)?,
                ControllerType::HugeTlb => HugeTlb::apply(linux_resources, &&full_cgroup_path)?,
                ControllerType::Io => Io::apply(linux_resources, &&full_cgroup_path)?,
                ControllerType::Memory => Memory::apply(linux_resources, &full_cgroup_path)?,
                ControllerType::Pids => Pids::apply(linux_resources, &&full_cgroup_path)?,
            }
        }

        Ok(())
    }

    fn remove(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_slice_works() -> Result<()> {
        let manager = SystemDCGroupManager::new(
            PathBuf::from("/sys/fs/cgroup"),
            PathBuf::from("test-a-b.slice:docker:foo"),
        )?;

        assert_eq!(
            manager.expand_slice("test-a-b.slice")?,
            PathBuf::from("/test.slice/test-a.slice/test-a-b.slice"),
        );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_a_complex_slice() -> Result<()> {
        let manager = SystemDCGroupManager::new(
            PathBuf::from("/sys/fs/cgroup"),
            PathBuf::from("test-a-b.slice:docker:foo"),
        )?;

        assert_eq!(
            manager.get_cgroups_path()?,
            PathBuf::from("/test.slice/test-a.slice/test-a-b.slice/docker-foo.scope"),
        );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_a_simple_slice() -> Result<()> {
        let manager = SystemDCGroupManager::new(
            PathBuf::from("/sys/fs/cgroup"),
            PathBuf::from("machine.slice:libpod:foo"),
        )?;

        assert_eq!(
            manager.get_cgroups_path()?,
            PathBuf::from("/machine.slice/libpod-foo.scope"),
        );

        Ok(())
    }

    #[test]
    fn get_cgroups_path_works_with_scope() -> Result<()> {
        let manager = SystemDCGroupManager::new(
            PathBuf::from("/sys/fs/cgroup"),
            PathBuf::from(":docker:foo"),
        )?;

        assert_eq!(
            manager.get_cgroups_path()?,
            PathBuf::from("/machine.slice/docker-foo.scope"),
        );

        Ok(())
    }
}
