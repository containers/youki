use std::path::Path;

use crate::common::{self, ControllerOpt, WrappedIoError};
use oci_spec::runtime::LinuxNetwork;

use super::controller::Controller;

pub struct NetworkClassifier {}

impl Controller for NetworkClassifier {
    type Error = WrappedIoError;
    type Resource = LinuxNetwork;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<(), Self::Error> {
        log::debug!("Apply NetworkClassifier cgroup config");

        if let Some(network) = Self::needs_to_handle(controller_opt) {
            Self::apply(cgroup_root, network)?;
        }

        Ok(())
    }

    fn needs_to_handle<'a>(controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        controller_opt.resources.network().as_ref()
    }
}

impl NetworkClassifier {
    fn apply(root_path: &Path, network: &LinuxNetwork) -> Result<(), WrappedIoError> {
        if let Some(class_id) = network.class_id() {
            common::write_cgroup_file(root_path.join("net_cls.classid"), class_id)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_temp_dir, set_fixture};
    use oci_spec::runtime::LinuxNetworkBuilder;

    #[test]
    fn test_apply_network_classifier() {
        let tmp = create_temp_dir("test_apply_network_classifier")
            .expect("create temp directory for test");
        set_fixture(&tmp, "net_cls.classid", "0").expect("set fixture for classID");

        let id = 0x100001u32;
        let network = LinuxNetworkBuilder::default()
            .class_id(id)
            .priorities(vec![])
            .build()
            .unwrap();

        NetworkClassifier::apply(&tmp, &network).expect("apply network classID");

        let content =
            std::fs::read_to_string(tmp.join("net_cls.classid")).expect("Read classID contents");
        assert_eq!(id.to_string(), content);
    }
}
