mod tests;
mod utils;

use crate::tests::lifecycle::{ContainerCreate, ContainerLifecycle};
use crate::tests::linux_ns_itype::get_ns_itype_tests;
use crate::tests::pidfile::get_pidfile_test;
use crate::tests::readonly_paths::get_ro_paths_test;
use crate::tests::seccomp_notify::get_seccomp_notify_test;
use crate::tests::tlb::get_tlb_test;
use crate::utils::support::set_runtime_path;
use anyhow::{Context, Result};
use clap::Parser;
use integration_test::logger;
use std::path::PathBuf;
use test_framework::TestManager;
use tests::cgroups;

#[derive(Parser, Debug)]
#[clap(version = "0.0.1", author = "youki team")]
struct Opts {
    /// Enables debug output
    #[clap(short, long)]
    debug: bool,

    #[clap(subcommand)]
    command: SubCommand,
}

#[derive(Parser, Debug)]
enum SubCommand {
    Run(Run),
    List,
}

#[derive(Parser, Debug)]
struct Run {
    /// Path for the container runtime to be tested
    #[clap(short, long)]
    runtime: PathBuf,
    /// Selected tests to be run, format should be
    /// space separated groups, eg
    /// -t group1::test1,test3 group2 group3::test5
    #[clap(short, long, multiple_values = true, value_delimiter = ' ')]
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

    if let Err(e) = logger::init(opts.debug) {
        eprintln!("logger could not be initialized: {:?}", e);
    }

    let mut tm = TestManager::new();

    let cl = ContainerLifecycle::new();
    let cc = ContainerCreate::new();
    let huge_tlb = get_tlb_test();
    let pidfile = get_pidfile_test();
    let ns_itype = get_ns_itype_tests();
    let cgroup_v1_pids = cgroups::pids::get_test_group();
    let cgroup_v1_cpu = cgroups::cpu::v1::get_test_group();
    let cgroup_v2_cpu = cgroups::cpu::v2::get_test_group();
    let cgroup_v1_memory = cgroups::memory::get_test_group();
    let cgroup_v1_network = cgroups::network::get_test_group();
    let cgroup_v1_blkio = cgroups::blkio::get_test_group();
    let seccomp_notify = get_seccomp_notify_test();
    let ro_paths = get_ro_paths_test();

    tm.add_test_group(&cl);
    tm.add_test_group(&cc);
    tm.add_test_group(&huge_tlb);
    tm.add_test_group(&pidfile);
    tm.add_test_group(&ns_itype);
    tm.add_test_group(&cgroup_v1_pids);
    tm.add_test_group(&cgroup_v1_cpu);
    tm.add_test_group(&cgroup_v2_cpu);
    tm.add_test_group(&cgroup_v1_memory);
    tm.add_test_group(&cgroup_v1_network);
    tm.add_test_group(&cgroup_v1_blkio);
    tm.add_test_group(&seccomp_notify);
    tm.add_test_group(&ro_paths);

    tm.add_cleanup(Box::new(cgroups::cleanup_v1));
    tm.add_cleanup(Box::new(cgroups::cleanup_v2));

    match &opts.command {
        SubCommand::Run(args) => run(args, &tm).context("run tests")?,
        SubCommand::List => list(&tm).context("list tests")?,
    }

    Ok(())
}

fn run(opts: &Run, test_manager: &TestManager) -> Result<()> {
    match std::fs::canonicalize(&opts.runtime) {
        // runtime path is relative or resolved correctly
        Ok(path) => set_runtime_path(&path),
        // runtime path is name of program which probably exists in $PATH
        Err(_) => match which::which(&opts.runtime) {
            Ok(path) => set_runtime_path(&path),
            Err(e) => {
                eprintln!("Error in finding runtime : {}\nexiting.", e);
                std::process::exit(66);
            }
        },
    }

    if let Some(tests) = &opts.tests {
        let tests_to_run = parse_tests(tests);
        test_manager.run_selected(tests_to_run);
    } else {
        test_manager.run_all();
    }

    Ok(())
}

fn list(test_manager: &TestManager) -> Result<()> {
    for test_group in test_manager.tests_groups() {
        println!("{}", test_group);
    }

    Ok(())
}
