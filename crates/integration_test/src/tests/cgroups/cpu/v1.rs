use std::path::Path;

use num_cpus;
use test_framework::{test_result, ConditionalTest, TestGroup, TestResult};

use crate::utils::{test_outside_container, test_utils::check_container_created};

use super::{create_cpu_spec, create_empty_spec, create_spec};

const CPU_CGROUP_PREFIX: &str = "/sys/fs/cgroup/cpu,cpuacct";
const DEFAULT_REALTIME_PERIOD: u64 = 1000000;
const DEFAULT_REALTIME_RUNTIME: i64 = 950000;

fn get_realtime_period() -> Option<u64> {
    if Path::new(CPU_CGROUP_PREFIX)
        .join("cpu.rt_period_us")
        .exists()
    {
        return Some(DEFAULT_REALTIME_PERIOD);
    }
    None
}

fn get_realtime_runtime() -> Option<i64> {
    if Path::new(CPU_CGROUP_PREFIX)
        .join("cpu.rt_runtime_us")
        .exists()
    {
        return Some(DEFAULT_REALTIME_RUNTIME);
    }
    None
}

fn test_cpu_cgroups() -> TestResult {
    let cgroup_name = "test_cpu_cgroups";
    // Kernel counts 0 as a CPU, so on a system with 8 logical cores you will need `0-7` range set.
    let cpu_range = format!("0-{}", num_cpus::get() - 1);

    let realtime_period = get_realtime_period();
    let realtime_runtime = get_realtime_runtime();

    let cases = vec![
        test_result!(create_cpu_spec(
            1024,
            100000,
            50000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            100000,
            50000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            100000,
            200000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            100000,
            200000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            500000,
            50000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            500000,
            50000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            500000,
            200000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            1024,
            500000,
            200000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            100000,
            50000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            100000,
            50000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            100000,
            200000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            100000,
            200000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            500000,
            50000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            500000,
            50000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            500000,
            200000,
            "0",
            "0",
            realtime_period,
            realtime_runtime,
        )),
        test_result!(create_cpu_spec(
            2048,
            500000,
            200000,
            &cpu_range,
            "0",
            realtime_period,
            realtime_runtime,
        )),
    ];

    for case in cases.into_iter() {
        let spec = test_result!(create_spec(cgroup_name, case));
        let test_result = test_outside_container(spec, &|data| {
            test_result!(check_container_created(&data));

            TestResult::Passed
        });

        if let TestResult::Failed(_) = test_result {
            return test_result;
        }
    }

    TestResult::Passed
}

fn test_empty_cpu() -> TestResult {
    let cgroup_name = "test_empty_cpu";
    let spec = test_result!(create_empty_spec(cgroup_name));

    test_outside_container(spec, &|data| {
        test_result!(check_container_created(&data));
        TestResult::Passed
    })
}

fn can_run() -> bool {
    Path::new(CPU_CGROUP_PREFIX).exists()
}

pub fn get_test_group<'a>() -> TestGroup<'a> {
    let mut test_group = TestGroup::new("cgroup_v1_cpu");
    let linux_cgroups_cpus = ConditionalTest::new(
        "test_linux_cgroups_cpus",
        Box::new(can_run),
        Box::new(test_cpu_cgroups),
    );

    let empty_cpu = ConditionalTest::new(
        "test_empty_cpu",
        Box::new(can_run),
        Box::new(test_empty_cpu),
    );

    test_group.add(vec![Box::new(linux_cgroups_cpus), Box::new(empty_cpu)]);

    test_group
}
