use std::cell::RefCell;

use anyhow::Result;
use nix::unistd::Pid;

use crate::{
    common::{CgroupManager, ControllerOpt, FreezerState},
    stats::Stats,
};

#[derive(Debug)]
pub struct TestManager {
    add_task_args: RefCell<Vec<Pid>>,
}

impl Default for TestManager {
    fn default() -> Self {
        Self {
            add_task_args: RefCell::new(vec![]),
        }
    }
}

impl CgroupManager for TestManager {
    fn add_task(&self, pid: Pid) -> Result<()> {
        self.add_task_args.borrow_mut().push(pid);
        Ok(())
    }

    // NOTE: The argument cannot be stored due to lifetime.
    fn apply(&self, _controller_opt: &ControllerOpt) -> Result<()> {
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        unimplemented!()
    }

    fn freeze(&self, _state: FreezerState) -> Result<()> {
        unimplemented!()
    }

    fn stats(&self) -> anyhow::Result<Stats> {
        unimplemented!()
    }

    fn get_all_pids(&self) -> Result<Vec<Pid>> {
        unimplemented!()
    }
}

impl TestManager {
    pub fn get_add_task_args(&self) -> Vec<Pid> {
        self.add_task_args.borrow_mut().clone()
    }
}
