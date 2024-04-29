use crate::utils::test_inside_container;
use oci_spec::runtime::{LinuxBuilder, ProcessBuilder, Spec, SpecBuilder};
use test_framework::{Test, TestGroup, TestResult};

fn create_spec(linux_mount_label: String) -> Spec {
    SpecBuilder::default()
        .linux(
            // Need to reset the read-only paths
            LinuxBuilder::default()
                .mount_label(linux_mount_label)
                .masked_paths(vec![])
                .build()
                .expect("error in building linux config"),
        )
        .process(
            ProcessBuilder::default()
                .args(vec![
                    "runtimetest".to_string(),
                    "linux_mount_label".to_string(),
                ])
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .unwrap()
}

// here we have to manually create and manage the container
// as the test_inside container does not provide a way to set the pid file argument
fn test_linux_mount_label() -> TestResult {
    let spec = create_spec("system_u:object_r:svirt_sandbox_file_t:s0:c715,c811".to_string());
    test_inside_container(spec, &|_| {
        // As long as the container is created, we expect the mount label to be determined
        // by the spec, so nothing to prepare prior.
        Ok(())
    })
}

pub fn get_linux_mount_label_test() -> TestGroup {
    let linux_mount_label = Test::new("linux_mount_label", Box::new(test_linux_mount_label));
    let mut tg = TestGroup::new("linux_mount_label");
    tg.add(vec![Box::new(linux_mount_label)]);
    tg
}
