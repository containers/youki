use std::io::prelude::*;
use std::{
    fs::{create_dir_all, OpenOptions},
    path::Path,
    thread, time,
};

use anyhow::{Result, *};

use crate::cgroups::common;
use crate::cgroups::v1::Controller;
use oci_spec::{FreezerState, LinuxResources};

const CGROUP_FREEZER_STATE: &str = "freezer.state";
const FREEZER_STATE_THAWED: &str = "THAWED";
const FREEZER_STATE_FROZEN: &str = "FROZEN";
const FREEZER_STATE_FREEZING: &str = "FREEZING";

pub struct Freezer {}

impl Controller for Freezer {
    type Resource = FreezerState;

    fn apply(linux_resources: &LinuxResources, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Freezer cgroup config");
        create_dir_all(&cgroup_root)?;

        if let Some(freezer_state) = Self::needs_to_handle(linux_resources) {
            Self::apply(freezer_state, cgroup_root)?;
        }

        Ok(())
    }

    fn needs_to_handle(linux_resources: &LinuxResources) -> Option<&Self::Resource> {
        if let Some(freezer_state) = &linux_resources.freezer {
            return Some(freezer_state);
        }

        None
    }
}

impl Freezer {
    fn apply(freezer_state: &FreezerState, cgroup_root: &Path) -> Result<()> {
        match freezer_state {
            FreezerState::Undefined => {}
            FreezerState::Thawed => {
                common::write_cgroup_file(
                    cgroup_root.join(CGROUP_FREEZER_STATE),
                    FREEZER_STATE_THAWED,
                )?;
            }
            FreezerState::Frozen => {
                let r = || -> Result<()> {
                    // We should do our best to retry if FREEZING is seen until it becomes FROZEN.
                    // Add sleep between retries occasionally helped when system is extremely slow.
                    // see:
                    // https://github.com/opencontainers/runc/blob/b9ee9c6314599f1b4a7f497e1f1f856fe433d3b7/libcontainer/cgroups/fs/freezer.go#L42
                    for i in 0..1000 {
                        if i % 50 == 49 {
                            let _ = common::write_cgroup_file(
                                cgroup_root.join(CGROUP_FREEZER_STATE),
                                FREEZER_STATE_THAWED,
                            );
                            thread::sleep(time::Duration::from_millis(10));
                        }

                        common::write_cgroup_file(
                            cgroup_root.join(CGROUP_FREEZER_STATE),
                            FREEZER_STATE_FROZEN,
                        )?;

                        if i % 25 == 24 {
                            thread::sleep(time::Duration::from_millis(10));
                        }

                        let r = Self::read_freezer_state(cgroup_root)?;
                        match r.trim() {
                            FREEZER_STATE_FREEZING => {
                                continue;
                            }
                            FREEZER_STATE_FROZEN => {
                                if i > 1 {
                                    log::debug!("frozen after {} retries", i)
                                }
                                return Ok(());
                            }
                            _ => {
                                // should not reach here.
                                bail!("unexpected state {} while freezing", r.trim());
                            }
                        }
                    }
                    bail!("unbale to freeze");
                }();

                if r.is_err() {
                    // Freezing failed, and it is bad and dangerous to leave the cgroup in FROZEN or
                    // FREEZING, so try to thaw it back.
                    let _ = common::write_cgroup_file(
                        cgroup_root.join(CGROUP_FREEZER_STATE),
                        FREEZER_STATE_THAWED,
                    );
                }
                return r;
            }
        }
        Ok(())
    }

    fn read_freezer_state(cgroup_root: &Path) -> Result<String> {
        let path = cgroup_root.join(CGROUP_FREEZER_STATE);
        let mut content = String::new();
        OpenOptions::new()
            .create(false)
            .read(true)
            .open(path)?
            .read_to_string(&mut content)?;
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::common::CGROUP_PROCS;
    use crate::cgroups::test::set_fixture;
    use crate::utils::create_temp_dir;
    use nix::unistd::Pid;
    use oci_spec::FreezerState;

    #[test]
    fn test_set_freezer_state() {
        let tmp =
            create_temp_dir("test_set_freezer_state").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_FREEZER_STATE, "").expect("Set fixure for freezer state");

        // set Frozen state.
        {
            let freezer_state = FreezerState::Frozen;
            Freezer::apply(&freezer_state, &tmp).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            assert_eq!(FREEZER_STATE_FROZEN, state_content);
        }

        // set Thawed state.
        {
            let freezer_state = FreezerState::Thawed;
            Freezer::apply(&freezer_state, &tmp).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            assert_eq!(FREEZER_STATE_THAWED, state_content);
        }

        // set Undefined state.
        {
            let old_state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            let freezer_state = FreezerState::Undefined;
            Freezer::apply(&freezer_state, &tmp).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            assert_eq!(old_state_content, state_content);
        }
    }

    #[test]
    fn test_add_and_apply() {
        let tmp = create_temp_dir("test_add_task").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_FREEZER_STATE, "").expect("set fixure for freezer state");
        set_fixture(&tmp, CGROUP_PROCS, "").expect("set fixture for proc file");

        // set Thawed state.
        {
            let linux_resources = LinuxResources {
                devices: vec![],
                disable_oom_killer: false,
                oom_score_adj: None,
                memory: None,
                cpu: None,
                pids: None,
                block_io: None,
                hugepage_limits: vec![],
                network: None,
                freezer: Some(FreezerState::Thawed),
            };

            let pid = Pid::from_raw(1000);
            Freezer::add_task(pid, &tmp).expect("freezer add task");
            <Freezer as Controller>::apply(&linux_resources, &tmp).expect("freezer apply");
            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            assert_eq!(FREEZER_STATE_THAWED, state_content);
            let pid_content =
                std::fs::read_to_string(tmp.join(CGROUP_PROCS)).expect("read to string");
            assert_eq!(pid_content, "1000");
        }

        // set Frozen state.
        {
            let linux_resources = LinuxResources {
                devices: vec![],
                disable_oom_killer: false,
                oom_score_adj: None,
                memory: None,
                cpu: None,
                pids: None,
                block_io: None,
                hugepage_limits: vec![],
                network: None,
                freezer: Some(FreezerState::Frozen),
            };

            let pid = Pid::from_raw(1001);
            Freezer::add_task(pid, &tmp).expect("freezer add task");
            <Freezer as Controller>::apply(&linux_resources, &tmp).expect("freezer apply");
            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            assert_eq!(FREEZER_STATE_FROZEN, state_content);
            let pid_content =
                std::fs::read_to_string(tmp.join(CGROUP_PROCS)).expect("read to string");
            assert_eq!(pid_content, "1001");
        }

        // set Undefined state.
        {
            let linux_resources = LinuxResources {
                devices: vec![],
                disable_oom_killer: false,
                oom_score_adj: None,
                memory: None,
                cpu: None,
                pids: None,
                block_io: None,
                hugepage_limits: vec![],
                network: None,
                freezer: Some(FreezerState::Undefined),
            };

            let pid = Pid::from_raw(1002);
            let old_state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            Freezer::add_task(pid, &tmp).expect("freezer add task");
            <Freezer as Controller>::apply(&linux_resources, &tmp).expect("freezer apply");
            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            assert_eq!(old_state_content, state_content);
            let pid_content =
                std::fs::read_to_string(tmp.join(CGROUP_PROCS)).expect("read to string");
            assert_eq!(pid_content, "1002");
        }
    }
}
