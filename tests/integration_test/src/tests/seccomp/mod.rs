use oci_spec::runtime::{
    Arch, LinuxBuilder, LinuxSeccomp, LinuxSeccompAction, LinuxSeccompBuilder, LinuxSyscallBuilder,
    ProcessBuilder, Spec, SpecBuilder,
};
use test_framework::{Test, TestGroup, TestResult};

use crate::utils::test_inside_container;

fn create_spec(seccomp: LinuxSeccomp) -> Spec {
    SpecBuilder::default()
        .linux(
            LinuxBuilder::default()
                .seccomp(seccomp)
                .build()
                .expect("error in building linux config"),
        )
        .process(
            ProcessBuilder::default()
                .args(vec!["runtimetest".to_string(), "seccomp".to_string()])
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .unwrap()
}

fn seccomp_test() -> TestResult {
    let spec = create_spec(
        LinuxSeccompBuilder::default()
            .default_action(LinuxSeccompAction::ScmpActAllow)
            .syscalls(vec![LinuxSyscallBuilder::default()
                .names(vec![String::from("getcwd")])
                .action(LinuxSeccompAction::ScmpActErrno)
                .build()
                .unwrap()])
            .build()
            .unwrap(),
    );
    test_inside_container(spec, &|_| Ok(()))
}

pub fn get_seccomp_test() -> TestGroup {
    let mut test_group = TestGroup::new("seccomp");
    let seccomp_test = Test::new("seccomp_test", Box::new(seccomp_test));
    test_group.add(vec![Box::new(seccomp_test)]);

    test_group
}
