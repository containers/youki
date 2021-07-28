use crate::cgroups::v1::Controller;
use anyhow::Result;
use oci_spec::LinuxResources;
use std::path::Path;

pub struct PerfEvent {}

impl Controller for PerfEvent {
    type Resource = ();

    fn apply(_linux_resources: &LinuxResources, _cgroup_root: &Path) -> Result<()> {
        Ok(())
    }
    //no need to handle any case
    fn needs_to_handle(_linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use nix::unistd::Pid;

    use super::*;
    use crate::cgroups::{common::CGROUP_PROCS, test::setup};

    #[test]
    fn test_add_task() {
        let (tmp, procs) = setup("test_perf_event_add_task", CGROUP_PROCS);
        let pid = Pid::from_raw(1000);

        PerfEvent::add_task(pid, &tmp).expect("apply perf_event");

        let content = fs::read_to_string(&procs)
            .unwrap_or_else(|_| panic!("read {} file content", CGROUP_PROCS));
        assert_eq!(content, "1000");
    }
}
