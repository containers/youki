use crate::utils::test_outside_container;
use anyhow::anyhow;
use oci_spec::runtime::LinuxBuilder;
use oci_spec::runtime::{Spec, SpecBuilder};
use procfs::process::Process;
use test_framework::{Test, TestGroup, TestResult};

// I'm not sure we even need this
//const NAMESPACES: [&str; 5] = ["pid", "net", "ipc", "uts", "mnt"];

// get spec for the test
fn get_spec() -> Spec {
    let r = SpecBuilder::default()
        // We need to remove hostname to avoid test failures when not creating UTS namespace
        .hostname("")
        .linux(
            LinuxBuilder::default()
                .namespaces(
                    // the original test essientially skips all default namespaces,
                    // so we just put an empty vec here
                    vec![],
                )
                // if these both are not empty, we cannot set a inherited
                // mnt namespace, as these both require a private mnt namespace
                // original test config has these empty by default, and has a function
                // to add them if required
                .masked_paths(vec![])
                .readonly_paths(vec![])
                .build()
                .expect("could not build spec"),
        )
        .build()
        .unwrap();
    r
}

fn get_test<'a>(test_name: &'static str) -> Test<'a> {
    Test::new(
        test_name,
        Box::new(move || {
            let host_proc = Process::myself().unwrap();
            let host_namespaces = match host_proc.namespaces() {
                Ok(n) => n,
                Err(e) => {
                    return TestResult::Err(anyhow!("Error in resolving host namespaces : {}", e))
                }
            };
            // ! we don't have to actually store these separately
            // ! as we are making all namespaces to be inherited, we can directly compare
            // ! the hashmaps (?)
            // let mut ns_inode = HashMap::new();
            // for ns in namespaces {
            //     ns_inode.insert(ns.ns_type.into_string().unwrap(), ns.identifier);
            // }

            let spec = get_spec();
            test_outside_container(spec, &move |data| {
                let pid = data.state.unwrap().pid.unwrap();
                let container_process = Process::new(pid).unwrap();
                let container_namespaces = container_process.namespaces().unwrap();

                // for ns in namespaces {
                //     let inode = ns_inode
                //         .get(&ns.ns_type.clone().into_string().unwrap())
                //         .unwrap();

                // ! directly compare the hashmaps
                if container_namespaces != host_namespaces {
                    return TestResult::Err(anyhow!(
                        "Error : namespaces are not correctly inherited"
                    ));
                }
                // }
                TestResult::Ok
            })
        }),
    )
}

pub fn get_ns_itype_tests<'a>() -> TestGroup<'a> {
    let mut tg = TestGroup::new("ns_itype");
    let tests: Vec<_> = vec![Box::new(get_test("ns_itype"))];
    tg.add(tests);
    tg
}
