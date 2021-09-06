use anyhow::{bail, Context, Result};
use nix::{sys::signal, unistd::Pid};
use oci_spec::Hook;
use std::{
    collections::HashMap, fmt, io::ErrorKind, io::Write, os::unix::prelude::CommandExt, process,
    thread, time,
};

use crate::{container::Container, utils};
// A special error used to signal a timeout. We want to differenciate between a
// timeout vs. other error.
#[derive(Debug)]
pub struct HookTimeoutError;
impl std::error::Error for HookTimeoutError {}
impl fmt::Display for HookTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "hook command timeout".fmt(f)
    }
}

pub fn run_hooks(hooks: Option<&Vec<Hook>>, container: Option<&Container>) -> Result<()> {
    if container.is_none() {
        bail!("container state is required to run hook");
    }

    let state = &container.unwrap().state;

    if let Some(hooks) = hooks {
        for hook in hooks {
            let mut hook_command = process::Command::new(&hook.path);
            // Based on OCI spec, the first arguement of the args vector is the
            // arg0, which can be different from the path.  For example, path
            // may be "/usr/bin/true" and arg0 is set to "true". However, rust
            // command differenciates arg0 from args, where rust command arg
            // doesn't include arg0. So we have to make the split arg0 from the
            // rest of args.
            if let Some((arg0, args)) = hook.args.as_ref().map(|a| a.split_first()).flatten() {
                log::debug!("run_hooks arg0: {:?}, args: {:?}", arg0, args);
                hook_command.arg0(arg0).args(args)
            } else {
                hook_command.arg0(&hook.path.as_path().display().to_string())
            };

            let envs: HashMap<String, String> = if let Some(env) = hook.env.as_ref() {
                utils::parse_env(env)
            } else {
                HashMap::new()
            };
            log::debug!("run_hooks envs: {:?}", envs);

            let mut hook_process = hook_command
                .env_clear()
                .envs(envs)
                .stdin(process::Stdio::piped())
                .spawn()
                .with_context(|| "Failed to execute hook")?;
            let hook_process_pid = Pid::from_raw(hook_process.id() as i32);
            // Based on the OCI spec, we need to pipe the container state into
            // the hook command through stdin.
            if let Some(mut stdin) = hook_process.stdin.as_ref() {
                // We want to ignore BrokenPipe here. A BrokenPipe indicates
                // either the hook is crashed/errored or it ran successfully.
                // Either way, this is an indication that the hook command
                // finished execution.  If the hook command was successful,
                // which we will check later in this function, we should not
                // fail this step here. We still want to check for all the other
                // error, in the case that the hook command is waiting for us to
                // write to stdin.
                let encoded_state =
                    serde_json::to_string(state).context("Failed to encode container state")?;
                if let Err(e) = stdin.write_all(encoded_state.as_bytes()) {
                    if e.kind() != ErrorKind::BrokenPipe {
                        // Not a broken pipe. The hook command may be waiting
                        // for us.
                        let _ = signal::kill(hook_process_pid, signal::Signal::SIGKILL);
                        bail!("Failed to write container state to stdin: {:?}", e);
                    }
                }
            }

            let res = if let Some(timeout_sec) = hook.timeout {
                // Rust does not make it easy to handle executing a command and
                // timeout. Here we decided to wait for the command in a
                // different thread, so the main thread is not blocked. We use a
                // channel shared between main thread and the wait thread, since
                // the channel has timeout functions out of the box. Rust won't
                // let us copy the Command structure, so we can't share it
                // between the wait thread and main thread. Therefore, we will
                // use pid to identify the process and send a kill signal. This
                // is what the Command.kill() does under the hood anyway. When
                // timeout, we have to kill the process and clean up properly.
                let (s, r) = crossbeam_channel::unbounded();
                thread::spawn(move || {
                    let res = hook_process.wait();
                    let _ = s.send(res);
                });
                match r.recv_timeout(time::Duration::from_secs(timeout_sec as u64)) {
                    Ok(res) => res,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Kill the process. There is no need to further clean
                        // up because we will be error out.
                        let _ = signal::kill(hook_process_pid, signal::Signal::SIGKILL);
                        return Err(HookTimeoutError.into());
                    }
                    Err(_) => {
                        unreachable!();
                    }
                }
            } else {
                hook_process.wait()
            };

            match res {
                Ok(exit_status) => match exit_status.code() {
                    Some(0) => {}
                    Some(exit_code) => {
                        bail!(
                            "Failed to execute hook command. Non-zero return code. {:?}",
                            exit_code
                        );
                    }
                    None => {
                        bail!("Process is killed by signal");
                    }
                },
                Err(e) => {
                    bail!("Failed to execute hook command: {:?}", e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::{bail, Result};
    use serial_test::serial;
    use std::path::PathBuf;

    #[test]
    #[serial]
    fn test_run_hook() -> Result<()> {
        {
            let default_container: Container = Default::default();
            run_hooks(None, Some(&default_container)).context("Failed simple test")?;
        }

        {
            let default_container: Container = Default::default();
            let hook = Hook {
                path: PathBuf::from("/bin/true"),
                args: None,
                env: None,
                timeout: None,
            };
            let hooks = Some(vec![hook]);
            run_hooks(hooks.as_ref(), Some(&default_container)).context("Failed /bin/true")?;
        }

        {
            // Use `printenv` to make sure the environment is set correctly.
            let default_container: Container = Default::default();
            let hook = Hook {
                path: PathBuf::from("/usr/bin/bash"),
                args: Some(vec![
                    String::from("bash"),
                    String::from("-c"),
                    String::from("/usr/bin/printenv key > /dev/null"),
                ]),
                env: Some(vec![String::from("key=value")]),
                timeout: None,
            };
            let hooks = Some(vec![hook]);
            run_hooks(hooks.as_ref(), Some(&default_container)).context("Failed printenv test")?;
        }

        Ok(())
    }

    #[test]
    #[serial]
    #[ignore]
    // This will test executing hook with a timeout. Since the timeout is set in
    // secs, minimally, the test will run for 1 second to trigger the timeout.
    // Therefore, we leave this test in the normal execution.
    fn test_run_hook_timeout() -> Result<()> {
        let default_container: Container = Default::default();
        // We use `tail -f /dev/null` here to simulate a hook command that hangs.
        let hook = Hook {
            path: PathBuf::from("tail"),
            args: Some(vec![
                String::from("tail"),
                String::from("-f"),
                String::from("/dev/null"),
            ]),
            env: None,
            timeout: Some(1),
        };
        let hooks = Some(vec![hook]);
        match run_hooks(hooks.as_ref(), Some(&default_container)) {
            Ok(_) => {
                bail!("The test expects the hook to error out with timeout. Should not execute cleanly");
            }
            Err(err) => {
                // We want to make sure the error returned is indeed timeout
                // error. All other errors are considered failure.
                if !err.is::<HookTimeoutError>() {
                    bail!("Failed to execute hook: {:?}", err);
                }
            }
        }

        Ok(())
    }
}
