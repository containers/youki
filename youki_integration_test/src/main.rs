mod tests;
mod utils;

use crate::tests::lifecycle::{ContainerCreate, ContainerLifecycle};
use crate::tests::tlb::get_tlb_test;
use crate::utils::support::set_runtime_path;
use anyhow::Result;
use clap::Clap;
use std::path::PathBuf;
use test_framework::TestManager;

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

    match std::fs::canonicalize(opts.runtime.clone()) {
        // runtime path is relative or resolved correctly
        Ok(path) => set_runtime_path(&path),
        // runtime path is name of program which probably exists in $PATH
        Err(_) => match which::which(opts.runtime) {
            Ok(path) => set_runtime_path(&path),
            Err(e) => {
                eprintln!("Error in finding runtime : {}\nexiting.", e);
                std::process::exit(66);
            }
        },
    }

    let mut tm = TestManager::new();

    let cl = ContainerLifecycle::new();
    let cc = ContainerCreate::new();
    let huge_tlb = get_tlb_test();

    tm.add_test_group(&cl);
    tm.add_test_group(&cc);
    tm.add_test_group(&huge_tlb);

    if let Some(tests) = opts.tests {
        let tests_to_run = parse_tests(&tests);
        tm.run_selected(tests_to_run);
    } else {
        tm.run_all();
    }
    Ok(())
}
