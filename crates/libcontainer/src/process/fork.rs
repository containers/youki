use anyhow::{Context, Result};
use libc::SIGCHLD;
use nix::unistd::Pid;

// Fork/Clone a sibling process that shares the same parent as the calling
// process. This is used to launch the container init process so the parent
// process of the calling process can receive ownership of the process. If we
// clone a child process as the init process, the calling process (likely the
// youki main process) will exit and the init process will be re-parented to the
// process 1 (system init process), which is not the right behavior of what we
// look for.
pub fn container_clone_sibling<F: FnOnce() -> Result<i32>>(cb: F) -> Result<Pid> {
    let mut clone = clone3::Clone3::default();
    // Note: normally, an exit signal is required, but when using
    // `CLONE_PARENT`, the `clone3` will return EINVAL if an exit signal is set.
    // The older `clone` will not return EINVAL in this case. Instead it ignores
    // the exit signal bits in the glibc wrapper.
    clone.flag_parent();

    container_clone(cb, clone).with_context(|| "failed to clone sibling process")
}

// A simple clone wrapper to clone3 so we can share this logic in different
// fork/clone situations. We decided to minimally support kernel version >= 5.4,
// and `clone3` requires only kernel version >= 5.3. Therefore, we don't need to
// fall back to `clone` or `fork`.
fn container_clone<F: FnOnce() -> Result<i32>>(
    cb: F,
    mut clone_cmd: clone3::Clone3,
) -> Result<Pid> {
    // Return the child's pid in case of parent/calling process, and for the
    // cloned process, run the callback function, and exit with the same exit
    // code returned by the callback. If there was any error when trying to run
    // callback, exit with -1
    match unsafe { clone_cmd.call().with_context(|| "failed to run clone3")? } {
        0 => {
            // Inside the cloned process
            let ret = match cb() {
                Err(error) => {
                    log::debug!("failed to run child process in clone: {:?}", error);
                    -1
                }
                Ok(exit_code) => exit_code,
            };
            std::process::exit(ret);
        }
        pid => Ok(Pid::from_raw(pid)),
    }
}

// Execute the cb in another process. Make the fork works more like thread_spawn
// or clone, so it is easier to reason. Compared to clone call, fork is easier
// to use since fork will magically take care of all the variable copying. If
// using clone, we would have to manually make sure all the variables are
// correctly send to the new process, especially Rust borrow checker will be a
// lot of hassel to deal with every details.
pub fn container_fork<F: FnOnce() -> Result<i32>>(cb: F) -> Result<Pid> {
    // Using `clone3` to mimic the effect of `fork`.
    let mut clone = clone3::Clone3::default();
    clone.exit_signal(SIGCHLD as u64);

    container_clone(cb, clone).with_context(|| "failed to fork process")
}

#[cfg(test)]
mod test {
    use crate::process::channel::channel;

    use super::*;
    use anyhow::{bail, Result};
    use nix::sys::wait::{waitpid, WaitStatus};
    use nix::unistd;

    #[test]
    fn test_container_fork() -> Result<()> {
        let pid = container_fork(|| Ok(0))?;
        match waitpid(pid, None).expect("wait pid failed.") {
            WaitStatus::Exited(p, status) => {
                assert_eq!(pid, p);
                assert_eq!(status, 0);
                Ok(())
            }
            _ => bail!("test failed"),
        }
    }

    #[test]
    fn test_container_err_fork() -> Result<()> {
        let pid = container_fork(|| bail!(""))?;
        match waitpid(pid, None).expect("wait pid failed.") {
            WaitStatus::Exited(p, status) => {
                assert_eq!(pid, p);
                assert_eq!(status, 255);
                Ok(())
            }
            _ => bail!("test failed"),
        }
    }

    #[test]
    fn test_container_clone_sibling() -> Result<()> {
        // The `container_clone_sibling` will create a sibling process (share
        // the same parent) of the calling process. In Unix, a process can only
        // wait on the immediate children process and can't wait on the sibling
        // process. Therefore, to test the logic, we will have to fork a process
        // first and then let the forked process call `container_clone_sibling`.
        // Then the testing process (the process where test is called), who are
        // the parent to this forked process and the sibling process cloned by
        // the `container_clone_sibling`, can wait on both processes.

        // We need to use a channel so that the forked process can pass the pid
        // of the sibling process to the testing process.
        let (sender, receiver) = &mut channel::<i32>()?;

        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                let sibling_process_pid =
                    Pid::from_raw(receiver.recv().with_context(|| {
                        "failed to receive the sibling pid from forked process"
                    })?);
                receiver.close()?;
                match waitpid(sibling_process_pid, None).expect("wait pid failed.") {
                    WaitStatus::Exited(p, status) => {
                        assert_eq!(sibling_process_pid, p);
                        assert_eq!(status, 0);
                    }
                    _ => bail!("failed to wait on the sibling process"),
                }
                // After sibling process exits, we can wait on the forked process.
                match waitpid(child, None).expect("wait pid failed.") {
                    WaitStatus::Exited(p, status) => {
                        assert_eq!(child, p);
                        assert_eq!(status, 0);
                    }
                    _ => bail!("failed to wait on the forked process"),
                }
            }
            unistd::ForkResult::Child => {
                // Inside the forked process. We call `container_clone` and pass
                // the pid to the parent process.
                let pid = container_clone_sibling(|| Ok(0))?;
                sender.send(pid.as_raw())?;
                sender.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }
}
