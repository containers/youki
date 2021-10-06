use crate::utils::test_outside_container;
use anyhow::anyhow;
use oci_spec::runtime::LinuxBuilder;
use oci_spec::runtime::{LinuxHugepageLimitBuilder, LinuxResourcesBuilder};
use oci_spec::runtime::{Spec, SpecBuilder};
use std::path::PathBuf;
use test_framework::{ConditionalTest, TestGroup, TestResult};

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
    test_outside_container(spec, &|data| {
        match data.exit_status {
            Err(e) => TestResult::Err(anyhow!(e)),
            Ok(res) => {
                if data.state.is_some() {
                    return TestResult::Err(anyhow!(
                        "stdout of state command was non-empty : {:?}",
                        data.state
                    ));
                }
                if data.state_err.is_empty() {
                    return TestResult::Err(anyhow!("stderr of state command was empty"));
                }
                if res.success() {
                    // The operation should not have succeeded as pagesize was not power of 2
                    TestResult::Err(anyhow!("Invalid page size of {} was allowed", page))
                } else {
                    TestResult::Ok
                }
            }
        }
    })
}

fn extract_page_size(dir_name: &str) -> String {
    let name_stripped = dir_name.strip_prefix("hugepages-").unwrap();
    let size = name_stripped.strip_suffix("kB").unwrap();
    let size: u64 = size.parse().unwrap();

    if size >= (1 << 20) {
        (size >> 20).to_string() + "GB"
    } else if size >= (1 << 10) {
        (size >> 10).to_string() + "MB"
    } else {
        size.to_string() + "KB"
    }
}

fn get_tlb_sizes() -> Vec<String> {
    let mut sizes = Vec::new();
    for hugetlb_entry in std::fs::read_dir("/sys/kernel/mm/hugepages").unwrap() {
        let hugetlb_entry = hugetlb_entry.unwrap();
        if !hugetlb_entry.path().is_dir() {
            continue;
        }

        let dir_name = hugetlb_entry.file_name();
        let dir_name = dir_name.to_str().unwrap();

        sizes.push(extract_page_size(dir_name));
    }
    sizes
}

fn validate_tlb(id: &str, size: &str, limit: i64) -> TestResult {
    let root = "/sys/fs/cgroup/hugetlb";
    let path = format!("{}/{}/hugetlb.{}.limit_in_bytes", root, id, size);
    let val_str = std::fs::read_to_string(&path).unwrap();
    let val: i64 = val_str.trim().parse().unwrap();
    if val == limit {
        TestResult::Ok
    } else {
        TestResult::Err(anyhow!(
            "Page limit not set correctly : for size {}, expected {}, got {}",
            size,
            limit,
            val
        ))
    }
}

fn test_valid_tlb() -> TestResult {
    // When setting the limit just for checking if writing works, the amount of memory
    // requested does not matter, as all insigned integers will be accepted.
    // Use 1GiB as an example
    let limit: i64 = 1 << 30;
    let tlb_sizes = get_tlb_sizes();
    for size in tlb_sizes.iter() {
        let spec = make_hugetlb_spec(size, limit);
        let res = test_outside_container(spec, &|data| {
            match data.exit_status {
                Err(e) => return TestResult::Err(anyhow!(e)),
                Ok(res) => {
                    if !data.state_err.is_empty() {
                        return TestResult::Err(anyhow!(
                            "stderr of state command was not-empty : {}",
                            data.state_err
                        ));
                    }
                    if data.state.is_none() {
                        return TestResult::Err(anyhow!("stdout of state command was invalid"));
                    }
                    let state = data.state.unwrap();
                    if state.id != data.id || state.status != "created" {
                        return TestResult::Err(anyhow!("invalid container state : expected id {} and status created, got id {} and state {}",data.id,state.id,state.status));
                    }
                    if !res.success() {
                        return TestResult::Err(anyhow!(
                            "Setting valid page size of {} was gave error",
                            size
                        ));
                    }
                }
            }
            let r = validate_tlb(&data.id, size, limit);
            if matches!(r, TestResult::Err(_)) {
                return r;
            }
            TestResult::Ok
        });
        if matches!(res, TestResult::Err(_)) {
            return res;
        }
    }
    TestResult::Ok
}

pub fn get_tlb_test<'a>() -> TestGroup<'a> {
    let wrong_tlb = ConditionalTest::new(
        "invalid_tlb",
        Box::new(check_hugetlb),
        Box::new(test_wrong_tlb),
    );
    let valid_tlb = ConditionalTest::new(
        "valid_tlb",
        Box::new(check_hugetlb),
        Box::new(test_valid_tlb),
    );
    let mut tg = TestGroup::new("huge_tlb");
    tg.add(vec![Box::new(wrong_tlb), Box::new(valid_tlb)]);
    tg
}
