use anyhow::{bail, Result};
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    str, thread,
    time::Duration,
};

use oci_spec::{FreezerState, LinuxResources};

use super::controller::Controller;

const CGROUP_FREEZE: &str = "cgroup.freeze";
const CGROUP_EVENTS: &str = "cgroup.events";

pub struct Freezer {}

impl Controller for Freezer {
    fn apply(linux_resources: &LinuxResources, cgroup_path: &Path) -> Result<()> {
        if let Some(freezer_state) = linux_resources.freezer {
            Self::apply(freezer_state, cgroup_path)?;
        }

        Ok(())
    }
}

impl Freezer {
    fn apply(freezer_state: FreezerState, path: &Path) -> Result<()> {
        let state_str = match freezer_state {
            FreezerState::Undefined => return Ok(()),
            FreezerState::Frozen => "1",
            FreezerState::Thawed => "0",
        };

        match OpenOptions::new()
            .create(false)
            .write(true)
            .open(path.join(CGROUP_FREEZE))
        {
            Err(e) => {
                if let FreezerState::Frozen = freezer_state {
                    bail!("freezer not supported {}", e);
                }
                return Ok(());
            }
            Ok(mut file) => file.write_all(state_str.as_bytes())?,
        };

        // confirm that the cgroup did actually change states.
        let actual_state = Self::read_freezer_state(path)?;
        if !actual_state.eq(&freezer_state) {
            bail!(
                "expected \"cgroup.freeze\" to be in state {:?} but was in {:?}",
                freezer_state,
                actual_state
            );
        }

        Ok(())
    }

    fn read_freezer_state(path: &Path) -> Result<FreezerState> {
        let mut buf = [0; 1];
        OpenOptions::new()
            .create(false)
            .read(true)
            .open(path.join(CGROUP_FREEZE))?
            .read_exact(&mut buf)?;

        let state = str::from_utf8(&buf)?;
        match state {
            "0" => Ok(FreezerState::Thawed),
            "1" => Self::wait_frozen(path),
            _ => bail!("unknown \"cgroup.freeze\" state: {}", state),
        }
    }

    // wait_frozen polls cgroup.events until it sees "frozen 1" in it.
    fn wait_frozen(path: &Path) -> Result<FreezerState> {
        let f = OpenOptions::new()
            .create(false)
            .read(true)
            .open(path.join(CGROUP_EVENTS))?;
        let mut f = BufReader::new(f);

        let wait_time = Duration::from_millis(10);
        let max_iter = 1000;
        let mut iter = 0;
        let mut line = String::new();

        loop {
            if iter == max_iter {
                bail!(
                    "timeout of {} ms reached waiting for the cgroup to freeze",
                    wait_time.as_millis() * max_iter
                );
            }
            line.clear();
            let num_bytes = f.read_line(&mut line)?;
            if num_bytes == 0 {
                break;
            }
            if line.starts_with("frozen ") {
                if line.starts_with("frozen 1") {
                    if iter > 1 {
                        log::debug!("frozen after {} retries", iter)
                    }
                    return Ok(FreezerState::Frozen);
                }
                iter += 1;
                thread::sleep(wait_time);
                f.seek(SeekFrom::Start(0))?;
            }
        }

        Ok(FreezerState::Undefined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::test::set_fixture;
    use crate::utils::create_temp_dir;
    use oci_spec::FreezerState;
    use std::sync::Arc;

    #[test]
    fn test_set_freezer_state() {
        let tmp = Arc::new(
            create_temp_dir("test_set_freezer_state").expect("create temp directory for test"),
        );
        set_fixture(&tmp, CGROUP_FREEZE, "").expect("Set fixure for freezer state");
        set_fixture(&tmp, CGROUP_EVENTS, "populated 0\nfrozen 0")
            .expect("Set fixure for freezer state");

        // set Frozen state.
        {
            // use another thread to update events file async.
            let p = Arc::clone(&tmp);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                set_fixture(&p, CGROUP_EVENTS, "populated 0\nfrozen 1")
                    .expect("Set fixure for freezer state");
            });
            let freezer_state = FreezerState::Frozen;
            Freezer::apply(freezer_state, &tmp).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZE)).expect("Read to string");
            assert_eq!("1", state_content);
        }

        // set Thawed state.
        {
            let freezer_state = FreezerState::Thawed;
            Freezer::apply(freezer_state, &tmp).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZE)).expect("Read to string");
            assert_eq!("0", state_content);
        }

        // set Undefined state.
        {
            let old_state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZE)).expect("Read to string");
            let freezer_state = FreezerState::Undefined;
            Freezer::apply(freezer_state, &tmp).expect("Set freezer state");

            let state_content =
                std::fs::read_to_string(tmp.join(CGROUP_FREEZE)).expect("Read to string");
            assert_eq!(old_state_content, state_content);
        }
    }

    #[test]
    fn test_set_freezer_state_error() {
        let tmp = create_temp_dir("test_set_freezer_state_error")
            .expect("create temp directory for test");
        set_fixture(&tmp, CGROUP_FREEZE, "").expect("Set fixure for freezer state");
        set_fixture(&tmp, CGROUP_EVENTS, "").expect("Set fixure for freezer state");

        // events file does not contain "frozen 1"
        {
            let freezer_state = FreezerState::Frozen;
            let r = Freezer::apply(freezer_state, &tmp);
            assert!(r.is_err());
        }
    }
}
