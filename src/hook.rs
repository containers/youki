use anyhow::{bail, Context, Result};
use nix::{sys::signal, unistd::Pid};
use oci_spec::Hook;
use std::{collections::HashMap, fmt, process, thread, time};

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

pub fn run_hooks(hooks: Option<Vec<Hook>>, container: Option<Container>) -> Result<()> {
    if let Some(hooks) = hooks {
        for hook in hooks {
            let envs: HashMap<String, String> = if let Some(env) = hook.env {
                utils::parse_env(env)
            } else {
                HashMap::new()
            };
            let mut hook_command = process::Command::new(hook.path)
                .args(hook.args.unwrap_or_default())
                .env_clear()
                .envs(envs)
                .stdin(if container.is_some() {
                    process::Stdio::piped()
                } else {
                    process::Stdio::null()
                })
                .stdout(process::Stdio::null())
                .stderr(process::Stdio::null())
                .spawn()
                .with_context(|| "Failed to execute hook")?;
            let hook_command_pid = Pid::from_raw(hook_command.id() as i32);
            // Based on the OCI spec, we need to pipe the container state into
            // the hook command through stdin.
            if hook_command.stdin.is_some() {
                let stdin = hook_command.stdin.take().unwrap();
                if let Some(container) = &container {
                    serde_json::to_writer(stdin, &container.state)?;
                }
            }

            if let Some(timeout_sec) = hook.timeout {
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
                    let res = hook_command.wait();
                    let _ = s.send(res);
                });
                match r.recv_timeout(time::Duration::from_secs(timeout_sec as u64)) {
                    Ok(res) => {
                        match res {
                            Ok(exit_status) => {
                                if !exit_status.success() {
                                    bail!("Failed to execute hook command. Non-zero return code. {:?}", exit_status);
                                }
                            }
                            Err(e) => {
                                bail!("Failed to execute hook command: {:?}", e);
                            }
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Kill the process. There is no need to further clean
                        // up because we will be error out.
                        let _ = signal::kill(hook_command_pid, signal::Signal::SIGKILL);
                        return Err(HookTimeoutError.into());
                    }
                    Err(_) => {
                        unreachable!();
                    }
                }
            } else {
                hook_command.wait()?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::{bail, Result};
    use std::path::PathBuf;

    #[test]
    fn test_run_hook() -> Result<()> {
        run_hooks(None, None)?;

        {
            let default_container: Container = Default::default();
            let hook = Hook {
                path: PathBuf::from("/bin/true"),
                args: None,
                env: None,
                timeout: None,
            };
            let hooks = Some(vec![hook]);
            run_hooks(hooks, Some(default_container))?;
        }

        {
            // Use `printenv` to make sure the environment is set correctly.
            let default_container: Container = Default::default();
            let hook = Hook {
                path: PathBuf::from("/bin/printenv"),
                args: Some(vec!["key".to_string()]),
                env: Some(vec!["key=value".to_string()]),
                timeout: None,
            };
            let hooks = Some(vec![hook]);
            run_hooks(hooks, Some(default_container))?;
        }

        Ok(())
    }

    #[test]
    #[ignore]
    // This will test executing hook with a timeout. Since the timeout is set in
    // secs, minimally, the test will run for 1 second to trigger the timeout.
    // Therefore, we leave this test in the normal execution.
    fn test_run_hook_timeout() -> Result<()> {
        // We use `/bin/cat` here to simulate a hook command that hangs.
        let hook = Hook {
            path: PathBuf::from("tail"),
            args: Some(vec![String::from("-f"), String::from("/dev/null")]),
            env: None,
            timeout: Some(1),
        };
        let hooks = Some(vec![hook]);
        match run_hooks(hooks, None) {
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
