use anyhow::Result;
use nix::sched;
use nix::unistd::Pid;

pub fn clone(cb: sched::CloneCb, clone_flags: sched::CloneFlags) -> Result<Pid> {
    // unlike fork, clone requires the caller to allocate the stack. here, we use the default
    // 4KB for stack size, consistant with the runc implementation.
    const STACK_SIZE: usize = 4096;
    let stack: &mut [u8; STACK_SIZE] = &mut [0; STACK_SIZE];
    // pass in the SIGCHID flag to mimic the effect of forking a process
    let signal = nix::sys::signal::Signal::SIGCHLD;
    let pid = sched::clone(cb, stack, clone_flags, Some(signal as i32))?;

    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use nix::unistd;

    #[test]
    fn test_fork_clone() -> Result<()> {
        let cb = || -> Result<()> {
            // in a new pid namespace, pid of this process should be 1
            let pid = unistd::getpid();
            assert_eq!(unistd::Pid::from_raw(1), pid, "PID should set to 1");

            Ok(())
        };

        // For now, we test clone with new pid and user namespace. user
        // namespace is needed for the test to run without root
        let flags = sched::CloneFlags::CLONE_NEWPID | sched::CloneFlags::CLONE_NEWUSER;
        let pid = super::clone(
            Box::new(|| {
                if cb().is_err() {
                    return -1;
                }

                0
            }),
            flags,
        )?;

        let status = nix::sys::wait::waitpid(pid, None)?;
        if let nix::sys::wait::WaitStatus::Exited(_, exit_code) = status {
            assert_eq!(
                0, exit_code,
                "Process didn't exit correctly {:?}",
                exit_code
            );

            return Ok(());
        }

        bail!("Process didn't exit correctly")
    }
}
