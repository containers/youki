use std::path::Path;

use oci_spec::runtime::LinuxNetwork;

use super::controller::Controller;
use crate::common::{self, ControllerOpt, WrappedIoError};

pub struct NetworkClassifier {}

impl Controller for NetworkClassifier {
    type Error = WrappedIoError;
    type Resource = LinuxNetwork;

    fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<(), Self::Error> {
        tracing::debug!("Apply NetworkClassifier cgroup config");

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
    use oci_spec::runtime::LinuxNetworkBuilder;

    use super::*;
    use crate::test::set_fixture;

    #[test]
    fn test_apply_network_classifier() {
        let tmp = tempfile::tempdir().unwrap();
        set_fixture(tmp.path(), "net_cls.classid", "0").expect("set fixture for classID");

        let id = 0x100001u32;
        let network = LinuxNetworkBuilder::default()
            .class_id(id)
            .priorities(vec![])
            .build()
            .unwrap();

        NetworkClassifier::apply(tmp.path(), &network).expect("apply network classID");

        let content = std::fs::read_to_string(tmp.path().join("net_cls.classid"))
            .expect("Read classID contents");
        assert_eq!(id.to_string(), content);
    }
}
