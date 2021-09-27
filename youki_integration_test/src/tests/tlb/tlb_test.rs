use crate::utils::{
    generate_uuid, get_state, prepare_bundle, set_config, start_runtime, stop_runtime,
};
use anyhow::anyhow;
use oci_spec::runtime::LinuxBuilder;
use oci_spec::runtime::{LinuxHugepageLimitBuilder, LinuxResourcesBuilder};
use oci_spec::runtime::{Spec, SpecBuilder};
use test_framework::{ConditionalTest, TestGroup, TestResult};

use std::path::PathBuf;

fn check_hugetlb() -> bool {
    PathBuf::from("/sys/fs/cgroup/hugetlb").exists()
}

fn make_hugetlb_spec(page_size: &str, limit: i64) -> Spec {
    SpecBuilder::default()
        .linux(
            LinuxBuilder::default()
                .resources(
                    LinuxResourcesBuilder::default()
                        .hugepage_limits(vec![LinuxHugepageLimitBuilder::default()
                            .page_size(page_size.to_owned())
                            .limit(limit)
                            .build()
                            .expect("Could not build")])
                        .build()
                        .unwrap(),
                )
                .build()
                .expect("could not build"),
        )
        .build()
        .unwrap()
}

fn test_wrong_tlb() -> TestResult {
    // 3 MB pagesize is wrong, as valid values must be a power of 2
    let page = "3MB";
    let limit = 100 * 3 * 1024 * 1024;
    let spec = make_hugetlb_spec(page, limit);

    let id = generate_uuid();
    let bundle = prepare_bundle(&id).unwrap();
    set_config(&bundle, &spec).unwrap();

    let r = start_runtime(&id, &bundle).unwrap().wait();
    let (out, err) = get_state(&id, &bundle).unwrap();
    stop_runtime(&id, &bundle).unwrap().wait().unwrap();

    match r {
        Err(e) => TestResult::Err(anyhow!(e)),
        Ok(res) => {
            if !out.is_empty() {
                return TestResult::Err(anyhow!("stdout of state command was non-empty : {}", out));
            }
            if err.is_empty() {
                return TestResult::Err(anyhow!("stderr of state command was empty : {}", out));
            }
            if res.success() {
                // The operation should not have succeeded as pagesize was not power of 2
                return TestResult::Err(anyhow!("Invalid page size of {} was allowed", page));
            } else {
                TestResult::Ok
            }
        }
    }
}

pub fn tlb_test<'a>() -> TestGroup<'a> {
    let wrong_tlb = ConditionalTest::new(
        "wrong_tlb",
        Box::new(check_hugetlb),
        Box::new(test_wrong_tlb),
    );
    let mut tg = TestGroup::new("huge_tlb");
    tg.add(vec![Box::new(wrong_tlb)]);
    tg
}
