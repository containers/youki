mod support;
mod tests;

use anyhow::{bail, Result};
use clap::Clap;
use std::path::PathBuf;
use test_framework::TestManager;

use crate::support::cleanup_test;
use crate::support::get_project_path;
use crate::support::initialize_test;
use crate::tests::lifecycle::{ContainerCreate, ContainerLifecycle};

#[derive(Clap, Debug)]
#[clap(version = "0.0.1", author = "youki team")]
struct Opts {
    /// path for the container runtime to be tested
    #[clap(short, long)]
    runtime: PathBuf,
    #[clap(short, long, multiple = true, value_delimiter = " ")]
    tests: Option<Vec<String>>,
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    println!("{:?}", opts);

    // let mut tm = TestManager::new();
    // let project_path = get_project_path();

    // let cl = ContainerLifecycle::new(&project_path);
    // let cc = ContainerCreate::new(&project_path);

    // tm.add_test_group(&cl);
    // tm.add_test_group(&cc);

    // if initialize_test(&project_path).is_err() {
    //     bail!("Can not initilize test.")
    // }

    // tm.run_all();

    // if cleanup_test(&project_path).is_err() {
    //     bail!("Can not cleanup test.")
    // }
    Ok(())
}
