use anyhow::{bail, Result};

mod support;
mod tests;

use crate::support::cleanup_test;
use crate::support::get_project_path;
use crate::support::initialize_test;
use crate::tests::lifecycle::{ContainerCreate, ContainerLifecycle};
use test_framework::TestManager;

fn main() -> Result<()> {
    let mut tm = TestManager::new();
    let project_path = get_project_path();

    let cl = ContainerLifecycle::new(&project_path);
    let cc = ContainerCreate::new(&project_path);

    tm.add_test_group(&cl);
    tm.add_test_group(&cc);

    if initialize_test(&project_path).is_err() {
        bail!("Can not initilize test.")
    }

    tm.run_all();

    if cleanup_test(&project_path).is_err() {
        bail!("Can not cleanup test.")
    }
    Ok(())
}
