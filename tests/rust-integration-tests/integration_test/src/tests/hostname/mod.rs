use anyhow::{Context, Result};
use oci_spec::runtime::{LinuxBuilder, ProcessBuilder, Spec, SpecBuilder};
use test_framework::{Test, TestGroup, TestResult};

use crate::utils::test_inside_container;

fn create_spec(hostname: &str) -> Result<Spec> {
    Ok(SpecBuilder::default()
        .hostname(hostname)
        .linux(
            LinuxBuilder::default()
                .readonly_paths(vec![
                    "/proc/bus".to_string(),
                    "/proc/fs".to_string(),
                    "/proc/sys".to_string(),
                ])
                .build()
                .expect("should build linux config"),
        )
        .process(
            ProcessBuilder::default()
                .args(vec!["runtimetest".to_string()])
                .build()
                .expect("can create process"),
        )
        .build()
        .context("Failed to build hostname spec")?)
}

fn hostname_test() -> TestResult {
    let cases = vec!["hostname-specific", ""];
    for case in cases {
        let spec = create_spec(case).expect("should create spec");
        let test_result = test_inside_container(spec, &|_| {
            // As long as the container is created, we expect the hostname to be determined
            // by the spec, so nothing to prepare prior.
            Ok(())
        });
        // fail fast
        if let TestResult::Failed(e) = test_result {
            return TestResult::Failed(e);
        }
    }
    TestResult::Passed
}

pub fn get_hostname_test() -> TestGroup {
    let mut test_group = TestGroup::new("set_host_name");
    let hostname_test = Test::new("set_host_name_test", Box::new(hostname_test));
    test_group.add(vec![Box::new(hostname_test)]);

    test_group
}
