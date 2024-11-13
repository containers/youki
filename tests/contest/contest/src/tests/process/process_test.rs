use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{bail, Context, Ok, Result};
use oci_spec::runtime::{ProcessBuilder, Spec, SpecBuilder};
use test_framework::{test_result, Test, TestGroup, TestResult};

use crate::utils::test_inside_container;

fn create_spec() -> Result<Spec> {
    let mut spec = SpecBuilder::default()
        .process(
            ProcessBuilder::default()
                .args(vec!["runtimetest".to_string(), "process".to_string()])
                .cwd("/test")
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .context("failed to build spec")?;
    spec.process()
        .set_env(vec!["testa=valuea".into(), "testb=123".into()]);
    Ok(spec)
}

fn process_test() -> TestResult {
    let spec = test_result!(create_spec());

    test_inside_container(spec, &|bundle| {
        match fs::create_dir(bundle.join("test")) {
            Result::Ok(_) => { /*This is expected*/ }
            Err(e) => {
                bail!(e)
            }
        }
        let metadata = fs::metadata(bundle.join("test"))?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);

        Ok(())
    })
}

pub fn get_process_test() -> TestGroup {
    let mut process_test_group = TestGroup::new("process");

    let test = Test::new("process_test", Box::new(process_test));
    process_test_group.add(vec![Box::new(test)]);

    process_test_group
}
