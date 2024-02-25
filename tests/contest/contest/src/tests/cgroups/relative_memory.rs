use std::path::Path;

use crate::utils::{linux_resource_memory::validate_linux_resource_memory, test_outside_container};
use anyhow::{Context, Result};
use oci_spec::runtime::{
    LinuxBuilder, LinuxMemoryBuilder, LinuxResourcesBuilder, Spec, SpecBuilder,
};
use test_framework::{test_result, ConditionalTest, TestGroup, TestResult};

const CGROUP_MEMORY_LIMIT: &str = "/sys/fs/cgroup/memory/memory.limit_in_bytes";
const CGROUP_MEMORY_SWAPPINESS: &str = "/sys/fs/cgroup/memory/memory.swappiness";

const RELATIVE_CGROUPS_PATH: &str = "/testdir/runtime-test/container";

fn create_spec(cgroup_name: &str, limit: i64, swappiness: u64) -> Result<Spec> {
    let spec = SpecBuilder::default()
        .linux(
            LinuxBuilder::default()
                .cgroups_path(Path::new(RELATIVE_CGROUPS_PATH).join(cgroup_name))
                .resources(
                    LinuxResourcesBuilder::default()
                        .memory(
                            LinuxMemoryBuilder::default()
                                .limit(limit)
                                .swappiness(swappiness)
                                .build()
                                .context("failed to build memory spec")?,
                        )
                        .build()
                        .context("failed to build resource spec")?,
                )
                .build()
                .context("failed to build linux spec")?,
        )
        .build()
        .context("failed to build spec")?;

    Ok(spec)
}

fn test_relative_memory_cgroups() -> TestResult {
    let cgroup_name = "test_relative_memory_cgroups";

    let spec = test_result!(create_spec(cgroup_name, 50593792, 10));

    test_outside_container(spec.clone(), &|data| {
        test_result!(validate_linux_resource_memory(&spec, data));

        TestResult::Passed
    })
}

fn can_run() -> bool {
    Path::new(CGROUP_MEMORY_LIMIT).exists() && Path::new(CGROUP_MEMORY_SWAPPINESS).exists()
}

pub fn get_test_group() -> TestGroup {
    let mut test_group = TestGroup::new("cgroup_v1_relative_memory");
    let linux_cgroups_memory = ConditionalTest::new(
        "test_linux_cgroups_relative_memory",
        Box::new(can_run),
        Box::new(test_relative_memory_cgroups),
    );

    test_group.add(vec![Box::new(linux_cgroups_memory)]);

    test_group
}
