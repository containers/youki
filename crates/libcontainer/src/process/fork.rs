use anyhow::Result;
use nix::unistd;
use nix::unistd::Pid;

// Execute the cb in another process. Make the fork works more like thread_spawn
// or clone, so it is easier to reason. Compared to clone call, fork is easier
// to use since fork will magically take care of all the variable copying. If
// using clone, we would have to manually make sure all the variables are
// correctly send to the new process, especially Rust borrow checker will be a
// lot of hassel to deal with every details.
pub fn container_fork<F: FnOnce() -> Result<i32>>(cb: F) -> Result<Pid> {
    match unsafe { unistd::fork()? } {
        unistd::ForkResult::Parent { child } => Ok(child),
        unistd::ForkResult::Child => {
            let ret = match cb() {
                Err(error) => {
                    log::debug!("failed to run fork: {:?}", error);
                    -1
                }
                Ok(ec) => ec,
            };
            std::process::exit(ret);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::{bail, Result};
    use nix::sys::wait::{waitpid, WaitStatus};

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
}
