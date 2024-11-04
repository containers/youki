use crate::utils::test_inside_container;
use anyhow::{Context, Ok, Result};
use oci_spec::runtime::{ProcessBuilder, Spec, SpecBuilder, User};
use test_framework::{test_result, Test, TestGroup, TestResult};

fn create_spec() -> Result<Spec> {
    let user = User::default()
        .set_uid(10)
        .set_gid(10)
        .set_additional_gids(Option::from(!vec![5]))
        .set_umask(Option::from(!vec![0o02]));

    let spec = SpecBuilder::default()
        .process(
            ProcessBuilder::default()
                .user(user)
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .context("failed to build spec")?;
    Ok(spec)
}
fn process_user_test() -> TestResult {
    let spec = test_result!(create_spec());
    test_inside_container(spec, &|_| Ok(()))
}

pub fn get_process_user_test() -> TestGroup {
    let mut process_user_test_group = TestGroup::new("process_user");

    let test = Test::new("process_user_test", Box::new(process_user_test));
    process_user_test_group.add(vec![Box::new(test)]);

    process_user_test_group
}
