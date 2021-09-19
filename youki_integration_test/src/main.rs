mod support;
mod tests;

use anyhow::{bail, Result};
use clap::Clap;
use std::path::PathBuf;
use test_framework::TestManager;

use crate::support::cleanup_test;
use crate::support::get_project_path;
use crate::support::initialize_test;
use crate::support::set_runtime_path;
use crate::tests::lifecycle::{ContainerCreate, ContainerLifecycle};

#[derive(Clap, Debug)]
#[clap(version = "0.0.1", author = "youki team")]
struct Opts {
    /// path for the container runtime to be tested
    #[clap(short, long)]
    runtime: PathBuf,
    /// selected tests to be run, format should be
    /// space separated groups, eg
    /// -t group1::test1,test3 group2 group3::test5
    #[clap(short, long, multiple = true, value_delimiter = " ")]
    tests: Option<Vec<String>>,
}

// parse test string given in commandline option as pair of testgroup name and tests belonging to that
fn parse_tests(tests: &[String]) -> Vec<(&str, Option<Vec<&str>>)> {
    let mut ret = Vec::with_capacity(tests.len());
    for test in tests {
        if test.contains("::") {
            let (mod_name, test_names) = test.split_once("::").unwrap();
            let _tests = test_names.split(',').collect();
            ret.push((mod_name, Some(_tests)));
        } else {
            ret.push((test, None));
        }
    }
    ret
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let path = std::fs::canonicalize(opts.runtime).expect("Invalid runtime path");
    set_runtime_path(&path);

    let mut tm = TestManager::new();
    let project_path = get_project_path();

    let cl = ContainerLifecycle::new(&project_path);
    let cc = ContainerCreate::new(&project_path);

    tm.add_test_group(&cl);
    tm.add_test_group(&cc);

    if initialize_test(&project_path).is_err() {
        bail!("Can not initilize test.")
    }

    if let Some(tests) = opts.tests {
        let tests_to_run = parse_tests(&tests);
        tm.run_selected(tests_to_run);
    } else {
        tm.run_all();
    }

    if cleanup_test(&project_path).is_err() {
        bail!("Can not cleanup test.")
    }
    Ok(())
}
