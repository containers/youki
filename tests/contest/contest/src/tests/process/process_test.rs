use crate::utils::test_inside_container;
use anyhow::{bail, Context, Ok, Result};
use oci_spec::runtime::{ProcessBuilder, Spec, SpecBuilder};
use std::fs;
use test_framework::{test_result, Test, TestGroup, TestResult};

fn create_spec() -> Result<Spec> {
    let spec = SpecBuilder::default()
        .process(
            ProcessBuilder::default()
                .cwd("/test")
                .env(vec!["testa=valuea".into(), "testb=123".into()])
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .context("failed to build spec")?;

    Ok(spec)
}

fn process_test() -> TestResult {
    let spec = test_result!(create_spec());
    test_inside_container(spec, &|_| {
        match fs::create_dir("/test") {
            Result::Ok(_) => { /*This is expected*/ }
            Err(e) => {
                bail!(e)
            }
        }
        Ok(())
    })
}

pub fn get_process_test() -> TestGroup {
    let mut process_test_group = TestGroup::new("process");

    let test = Test::new("process_test", Box::new(process_test));
    process_test_group.add(vec![Box::new(test)]);

    process_test_group
}
