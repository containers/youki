use std::{fs::create_dir_all, path::Path, thread, time};

use anyhow::{Result, *};
use async_trait::async_trait;

use super::Controller;
use crate::common;
use crate::common::{ControllerOpt, FreezerState};

const CGROUP_FREEZER_STATE: &str = "freezer.state";
const FREEZER_STATE_THAWED: &str = "THAWED";
const FREEZER_STATE_FROZEN: &str = "FROZEN";
const FREEZER_STATE_FREEZING: &str = "FREEZING";

pub struct Freezer {}

#[async_trait(?Send)]
impl Controller for Freezer {
    type Resource = FreezerState;

    async fn apply(controller_opt: &ControllerOpt, cgroup_root: &Path) -> Result<()> {
        log::debug!("Apply Freezer cgroup config");
        create_dir_all(&cgroup_root)?;

        if let Some(freezer_state) = Self::needs_to_handle(controller_opt) {
            Self::apply(freezer_state, cgroup_root).await.context("failed to appyl freezer")?;
        }

        Ok(())
    }

    fn needs_to_handle<'a>(controller_opt: &'a ControllerOpt) -> Option<&'a Self::Resource> {
        controller_opt.freezer_state.as_ref()
    }
}

impl Freezer {
    async fn apply(freezer_state: &FreezerState, cgroup_root: &Path) -> Result<()> {
        match freezer_state {
            FreezerState::Undefined => {}
            FreezerState::Thawed => {
                common::write_cgroup_file(
                    cgroup_root.join(CGROUP_FREEZER_STATE),
                    FREEZER_STATE_THAWED,
                )?;
            }
            FreezerState::Frozen => {
                let r = Freezer::retry_freeze(cgroup_root).await;

                if r.is_err() {
                    // Freezing failed, and it is bad and dangerous to leave the cgroup in FROZEN or
                    // FREEZING, so try to thaw it back.
                    let _ = common::async_write_cgroup_file(
                        cgroup_root.join(CGROUP_FREEZER_STATE),
                        FREEZER_STATE_THAWED,
                    )
                    .await;
                }
                return r;
            }
        }
        Ok(())
    }

    async fn retry_freeze(cgroup_root: &Path) -> Result<()> {
        // We should do our best to retry if FREEZING is seen until it becomes FROZEN.
        // Add sleep between retries occasionally helped when system is extremely slow.
        // see:
        // https://github.com/opencontainers/runc/blob/b9ee9c6314599f1b4a7f497e1f1f856fe433d3b7/libcontainer/cgroups/fs/freezer.go#L42
        for i in 0..1000 {
            if i % 50 == 49 {
                let _ = common::async_write_cgroup_file(
                    cgroup_root.join(CGROUP_FREEZER_STATE),
                    FREEZER_STATE_THAWED,
                )
                    .await;
                thread::sleep(time::Duration::from_millis(10));
            }

            common::async_write_cgroup_file(
                cgroup_root.join(CGROUP_FREEZER_STATE),
                FREEZER_STATE_FROZEN,
            )
                .await?;

            if i % 25 == 24 {
                thread::sleep(time::Duration::from_millis(10));
            }

            let r = Self::read_freezer_state(cgroup_root).await?;
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
    }

    async fn read_freezer_state(cgroup_root: &Path) -> Result<String> {
        let path = cgroup_root.join(CGROUP_FREEZER_STATE);
        common::async_read_cgroup_file(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{FreezerState, CGROUP_PROCS};
    use crate::test::{create_temp_dir, set_fixture, aw};
    use nix::unistd::Pid;
    use oci_spec::runtime::LinuxResourcesBuilder;

    #[test]
    fn test_set_freezer_state() {
        let tmp =
            create_temp_dir("test_set_freezer_state").expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_FREEZER_STATE, "").expect("Set fixure for freezer state");

        // set Frozen state.
        {
            let freezer_state = FreezerState::Frozen;
            aw!(Freezer::apply(&freezer_state, &tmp)).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            assert_eq!(FREEZER_STATE_FROZEN, state_content);
        }

        // set Thawed state.
        {
            let freezer_state = FreezerState::Thawed;
            aw!(Freezer::apply(&freezer_state, &tmp)).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            assert_eq!(FREEZER_STATE_THAWED, state_content);
        }

        // set Undefined state.
        {
            let old_state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("Read to string");
            let freezer_state = FreezerState::Undefined;
            aw!(Freezer::apply(&freezer_state, &tmp)).expect("Set freezer state");

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
            let linux_resources = LinuxResourcesBuilder::default()
                .devices(vec![])
                .hugepage_limits(vec![])
                .build()
                .unwrap();
            let state = FreezerState::Thawed;

            let controller_opt = ControllerOpt {
                resources: &linux_resources,
                freezer_state: Some(state),
                oom_score_adj: None,
                disable_oom_killer: false,
            };

            let pid = Pid::from_raw(1000);
            Freezer::add_task(pid, &tmp).expect("freezer add task");
            aw!(<Freezer as Controller>::apply(&controller_opt, &tmp)).expect("freezer apply");
            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            assert_eq!(FREEZER_STATE_THAWED, state_content);
            let pid_content =
                std::fs::read_to_string(tmp.join(CGROUP_PROCS)).expect("read to string");
            assert_eq!(pid_content, "1000");
        }

        // set Frozen state.
        {
            let linux_resources = LinuxResourcesBuilder::default()
                .devices(vec![])
                .hugepage_limits(vec![])
                .build()
                .unwrap();
            let state = FreezerState::Frozen;

            let controller_opt = ControllerOpt {
                resources: &linux_resources,
                freezer_state: Some(state),
                oom_score_adj: None,
                disable_oom_killer: false,
            };

            let pid = Pid::from_raw(1001);
            Freezer::add_task(pid, &tmp).expect("freezer add task");
            aw!(<Freezer as Controller>::apply(&controller_opt, &tmp)).expect("freezer apply");
            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            assert_eq!(FREEZER_STATE_FROZEN, state_content);
            let pid_content =
                std::fs::read_to_string(tmp.join(CGROUP_PROCS)).expect("read to string");
            assert_eq!(pid_content, "1001");
        }

        // set Undefined state.
        {
            let linux_resources = LinuxResourcesBuilder::default()
                .devices(vec![])
                .hugepage_limits(vec![])
                .build()
                .unwrap();

            let state = FreezerState::Undefined;

            let controller_opt = ControllerOpt {
                resources: &linux_resources,
                freezer_state: Some(state),
                oom_score_adj: None,
                disable_oom_killer: false,
            };

            let pid = Pid::from_raw(1002);
            let old_state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            Freezer::add_task(pid, &tmp).expect("freezer add task");
            aw!(<Freezer as Controller>::apply(&controller_opt, &tmp)).expect("freezer apply");
            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZER_STATE)).expect("read to string");
            assert_eq!(old_state_content, state_content);
            let pid_content =
                std::fs::read_to_string(tmp.join(CGROUP_PROCS)).expect("read to string");
            assert_eq!(pid_content, "1002");
        }
    }
}
