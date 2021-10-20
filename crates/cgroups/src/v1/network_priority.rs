use std::path::Path;

use anyhow::{Context, Result};

use super::Controller;
use crate::common::{self, ControllerOpt};
use oci_spec::runtime::LinuxNetwork;

pub struct NetworkPriority {}

impl Controller for NetworkPriority {
    type Resource = LinuxNetwork;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply NetworkPriority cgroup config");

        if let Some(network) = Self::needs_to_handle(controller_opt) {
            Self::apply(cgroup_root, network)
                .context("failed to apply network priority resource restrictions")?;
        }

        Ok(())
    }

    fn needs_to_handle<'a>(controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        controller_opt.resources.network().as_ref()
    }
}

impl NetworkPriority {
    fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<()> {
        if let Some(ni_priorities) = network.priorities() {
            let priorities: String = ni_priorities.iter().map(|p| p.to_string()).collect();
            common::write_cgroup_file_str(root_path.join("net_prio.ifpriomap"), priorities.trim())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::{LinuxInterfacePriorityBuilder, LinuxNetworkBuilder};

    #[test]
    fn test_apply_network_priorites() {
        let tmp = create_temp_dir("test_apply_network_priorites")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_prio.ifpriomap", "").expect("set fixture for priority map");
        let priorities = vec![
            LinuxInterfacePriorityBuilder::default()
                .name("a")
                .priority(1u32)
                .build()
                .unwrap(),
            LinuxInterfacePriorityBuilder::default()
                .name("b")
                .priority(2u32)
                .build()
                .unwrap(),
        ];
        let priorities_string = priorities.iter().map(|p| p.to_string()).collect::<String>();
        let network = LinuxNetworkBuilder::default()
            .priorities(priorities)
            .build()
            .unwrap();

        NetworkPriority::apply(&tmp, &network).expect("apply network priorities");

        let content =
            std::fs::read_to_string(tmp.join("net_prio.ifpriomap")).expect("Read classID contents");
        assert_eq!(priorities_string.trim(), content);
    }
}
