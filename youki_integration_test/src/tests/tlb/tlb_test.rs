use crate::utils::{
    delete_container, generate_uuid, get_state, prepare_bundle, set_config, start_runtime,
    stop_runtime,
};
use anyhow::anyhow;
use oci_spec::runtime::LinuxBuilder;
use oci_spec::runtime::{LinuxHugepageLimitBuilder, LinuxResourcesBuilder};
use oci_spec::runtime::{Spec, SpecBuilder};
use std::io::Result;
use std::path::PathBuf;
use std::process::ExitStatus;
use test_framework::{ConditionalTest, TestGroup, TestResult};

struct RunData {
    id: uuid::Uuid,
    result: Result<ExitStatus>,
    stdout: String,
    stderr: String,
}

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

fn run_tlb_test(page: &str, limit: i64) -> RunData {
    let spec = make_hugetlb_spec(page, limit);

    let id = generate_uuid();
    let bundle = prepare_bundle(&id).unwrap();
    set_config(&bundle, &spec).unwrap();

    let r = start_runtime(&id, &bundle).unwrap().wait();
    let (out, err) = get_state(&id, &bundle).unwrap();
    stop_runtime(&id, &bundle).unwrap().wait().unwrap();
    delete_container(&id, &bundle).unwrap().wait().unwrap();
    RunData {
        id: id,
        result: r,
        stdout: out,
        stderr: err,
    }
}

fn test_wrong_tlb() -> TestResult {
    // 3 MB pagesize is wrong, as valid values must be a power of 2
    let page = "3MB";
    let limit = 100 * 3 * 1024 * 1024;
    let rdata = run_tlb_test(&page, limit);
    match rdata.result {
        Err(e) => TestResult::Err(anyhow!(e)),
        Ok(res) => {
            if !rdata.stdout.is_empty() {
                return TestResult::Err(anyhow!(
                    "stdout of state command was non-empty : {}",
                    rdata.stdout
                ));
            }
            if rdata.stderr.is_empty() {
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
}

fn extract_page_size(dir_name: &str) -> String {
    let name_stripped = dir_name.strip_prefix("hugepages-").unwrap();
    let size = name_stripped.strip_suffix("kB").unwrap();
    let size: u64 = size.parse().unwrap();

    let size_moniker = if size >= (1 << 20) {
        (size >> 20).to_string() + "GB"
    } else if size >= (1 << 10) {
        (size >> 10).to_string() + "MB"
    } else {
        size.to_string() + "KB"
    };
    return size_moniker;
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

        let id = generate_uuid();
        let bundle = prepare_bundle(&id).unwrap();
        set_config(&bundle, &spec).unwrap();

        let r = start_runtime(&id, &bundle).unwrap().wait();
        let (out, err) = get_state(&id, &bundle).unwrap();
        stop_runtime(&id, &bundle).unwrap().wait().unwrap();
        let rdata = RunData {
            id: id,
            result: r,
            stdout: out,
            stderr: err,
        };
        match rdata.result {
            Err(e) => return TestResult::Err(anyhow!(e)),
            Ok(res) => {
                if !rdata.stderr.is_empty() {
                    return TestResult::Err(anyhow!(
                        "stderr of state command was not-empty : {}",
                        rdata.stderr
                    ));
                }
                if rdata.stdout.is_empty() {
                    return TestResult::Err(anyhow!("stdout of state command was empty"));
                }
                if !rdata.stdout.contains(&format!(r#""id": "{}""#, rdata.id))
                    || !rdata.stdout.contains(r#""status": "created""#)
                {
                    todo!();
                    return TestResult::Err(anyhow!(""));
                }
                if !res.success() {
                    return TestResult::Err(anyhow!(
                        "Setting valid page size of {} was gave error",
                        size
                    ));
                }
            }
        }
        let r = validate_tlb(&rdata.id.to_string(), size, limit);
        if matches!(r, TestResult::Err(_)) {
            return r;
        }
        delete_container(&id, &bundle).unwrap().wait().unwrap();
    }
    TestResult::Ok
}

pub fn get_tlb_test<'a>() -> TestGroup<'a> {
    let wrong_tlb = ConditionalTest::new(
        "wrong_tlb",
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
