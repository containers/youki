use std::path::Path;

use super::controller::Controller;
use crate::common::{ControllerOpt, WrappedIoError};

pub struct PerfEvent {}

impl Controller for PerfEvent {
    type Error = WrappedIoError;
    type Resource = ();

    fn apply(_controller_opt: &ControllerOpt, _cgroup_root: &Path) -> Result<(), Self::Error> {
        Ok(())
    }
    //no need to handle any case
    fn needs_to_handle<'a>(_controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use nix::unistd::Pid;

    use super::*;
    use crate::common::CGROUP_PROCS;
    use crate::test::setup;

    #[test]
    fn test_add_task() {
        let (tmp, procs) = setup(CGROUP_PROCS);
        let pid = Pid::from_raw(1000);

        PerfEvent::add_task(pid, tmp.path()).expect("apply perf_event");

        let content = fs::read_to_string(procs)
            .unwrap_or_else(|_| panic!("read {CGROUP_PROCS} file content"));
        assert_eq!(content, "1000");
    }
}
