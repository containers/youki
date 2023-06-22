use libc::SIGCHLD;
use nix::unistd::Pid;
use prctl;

#[derive(Debug, thiserror::Error)]
pub enum CloneError {
    #[error("failed to clone process using clone3")]
    Clone(#[source] nix::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    #[error(transparent)]
    IntermediateProcess(
        #[from] crate::process::container_intermediate_process::IntermediateProcessError,
    ),
    #[error(transparent)]
    InitProcess(#[from] crate::process::container_init_process::InitProcessError),
    // Need a fake error for testing
    #[cfg(test)]
    #[error("unknown")]
    Test,
}

type Result<T> = std::result::Result<T, CloneError>;
type CallbackResult<T> = std::result::Result<T, CallbackError>;

// Fork/Clone a sibling process that shares the same parent as the calling
// process. This is used to launch the container init process so the parent
// process of the calling process can receive ownership of the process. If we
// clone a child process as the init process, the calling process (likely the
// youki main process) will exit and the init process will be re-parented to the
// process 1 (system init process), which is not the right behavior of what we
// look for.
pub fn container_clone_sibling<F: FnOnce() -> CallbackResult<i32>>(
    child_name: &str,
    cb: F,
) -> Result<Pid> {
    // Note: normally, an exit signal is required, but when using
    // `CLONE_PARENT`, the `clone3` will return EINVAL if an exit signal is set.
    // The older `clone` will not return EINVAL in this case. Instead it ignores
    // the exit signal bits in the glibc wrapper. Therefore, we explicitly set
    // the exit_signal to None here, so this works for both version of clone.
    container_clone(child_name, cb, libc::CLONE_PARENT as u64, None)
}

// Execute the cb in another process. Make the fork works more like thread_spawn
// or clone, so it is easier to reason. Compared to clone call, fork is easier
// to use since fork will magically take care of all the variable copying. If
// using clone, we would have to manually make sure all the variables are
// correctly send to the new process, especially Rust borrow checker will be a
// lot of hassel to deal with every details.
pub fn container_fork<F: FnOnce() -> CallbackResult<i32>>(child_name: &str, cb: F) -> Result<Pid> {
    container_clone(child_name, cb, 0, Some(SIGCHLD as u64))
}

// A simple clone wrapper to clone3 so we can share this logic in different
// fork/clone situations. We decided to minimally support kernel version >= 5.4,
// and `clone3` requires only kernel version >= 5.3. Therefore, we don't need to
// fall back to `clone` or `fork`.
fn container_clone<F: FnOnce() -> CallbackResult<i32>>(
    child_name: &str,
    cb: F,
    flags: u64,
    exit_signal: Option<u64>,
) -> Result<Pid> {
    // Return the child's pid in case of parent/calling process, and for the
    // cloned process, run the callback function, and exit with the same exit
    // code returned by the callback. If there was any error when trying to run
    // callback, exit with -1
    match clone_wrapper(flags, exit_signal) {
        -1 => Err(CloneError::Clone(nix::Error::last())),
        0 => {
            // Inside the cloned process
            prctl::set_name(child_name).expect("failed to set name");
            let ret = match cb() {
                Err(error) => {
                    tracing::debug!("failed to run child process in clone: {:?}", error);
                    -1
                }
                Ok(exit_code) => exit_code,
            };
            std::process::exit(ret);
        }
        pid => Ok(Pid::from_raw(pid)),
    }
}

#[repr(C)]
struct clone3_args {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

// clone_wrapper wraps the logic of using `clone3` with fallback behavior when
// `clone3` is either not available or blocked. While `libcontainer` maintains a
// minimum kernel version where `clone3` is available, we have found that in
// real life, places would choose to block `clone3`. This is mostly due to
// seccomp profile can't effectively filter `clone3` calls because the clone
// flags are inside the clone_args, not part of the variables like the `clone`
// call. Therefore, we try `clone3` first, but fallback to `clone` when ENOSYS
// is returned.
fn clone_wrapper(flags: u64, exit_signal: Option<u64>) -> i32 {
    let mut args = clone3_args {
        flags,
        pidfd: 0,
        child_tid: 0,
        parent_tid: 0,
        exit_signal: exit_signal.unwrap_or(0),
        stack: 0,
        stack_size: 0,
        tls: 0,
        set_tid: 0,
        set_tid_size: 0,
        cgroup: 0,
    };
    let args_ptr = &mut args as *mut clone3_args;
    let args_size = std::mem::size_of::<clone3_args>();
    // We strategically choose to use the raw syscall here because it is simpler
    // for our usecase. We don't have to care about all the other usecases that
    // clone syscalls supports in general.
    match unsafe { libc::syscall(libc::SYS_clone3, args_ptr, args_size) } {
        -1 if nix::Error::last() == nix::Error::ENOSYS => {
            // continue to fallback to clone syscall
        }
        ret => {
            return ret as i32;
        }
    };

    let ret = unsafe {
        // We choose to use the raw clone syscall here instead of the glibc
        // wrapper version for the following reasons:
        //
        // 1. the raw syscall behaves more like the fork and clone3 call, so the
        // substitution is more natural in the case of a fallback. We do not
        // need to create a new function for the child to execute. Like fork and
        // clone3, the clone raw syscall will start the child from the point of
        // clone call.
        //
        // 2. the raw clone syscall can take null or 0 for the child stack as
        // arguement. The syscall will do copy on write with the existing stack
        // and takes care of child stack allocation. Correctly allocate a child
        // stack is a pain when we previously implemented the logic using the
        // glibc clone wrapper.
        //
        // The strategically use of the raw clone syscall is safe here because
        // we are using a specific subset of the clone flags to launch
        // processes. Unlike the general clone syscall where a number of
        // usecases are supported such as launching thread, we want a behavior
        // that is more similar to fork.
        libc::syscall(
            libc::SYS_clone,
            flags | exit_signal.unwrap_or(0), // flags
            0,                                // stack
            0,                                // parent_tid
            0,                                // child_tid
            0,                                // tls
        )
    };

    ret as i32
}

#[cfg(test)]
mod test {
    use crate::channel::channel;

    use super::*;
    use anyhow::{bail, Context, Result};
    use nix::sys::wait::{waitpid, WaitStatus};
    use nix::unistd;

    #[test]
    fn test_container_fork() -> Result<()> {
        let pid = container_fork("test:child", || Ok(0))?;
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
        let pid = container_fork("test:child", || Err(CallbackError::Test))?;
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
                let pid = container_clone_sibling("test:child", || Ok(0))?;
                sender.send(pid.as_raw())?;
                sender.close()?;
                std::process::exit(0);
            }
        };

        Ok(())
    }

    // This test depends on libseccomp to work.
    #[cfg(feature = "libseccomp")]
    #[test]
    fn test_clone_fallback() -> Result<()> {
        use crate::test_utils::TestCallbackError;
        use oci_spec::runtime::{
            Arch, LinuxSeccompAction, LinuxSeccompBuilder, LinuxSyscallBuilder,
        };

        fn has_clone3() -> bool {
            // We use the probe syscall to check if the kernel supports clone3 or
            // seccomp has successfully blocked clone3.
            let res = unsafe { libc::syscall(libc::SYS_clone3, 0, 0) };
            let err = (res == -1)
                .then(std::io::Error::last_os_error)
                .expect("probe syscall should not succeed");
            err.raw_os_error() != Some(libc::ENOSYS)
        }

        // To test the fallback behavior, we will create a seccomp rule that
        // blocks `clone3` as ENOSYS.
        let syscall = LinuxSyscallBuilder::default()
            .names(vec![String::from("clone3")])
            .action(LinuxSeccompAction::ScmpActErrno)
            .errno_ret(libc::ENOSYS as u32)
            .build()?;
        let seccomp_profile = LinuxSeccompBuilder::default()
            .default_action(LinuxSeccompAction::ScmpActAllow)
            .architectures(vec![Arch::ScmpArchNative])
            .syscalls(vec![syscall])
            .build()?;

        crate::test_utils::test_in_child_process(|| {
            // We use seccomp to block `clone3`
            let _ = prctl::set_no_new_privileges(true);
            crate::seccomp::initialize_seccomp(&seccomp_profile)
                .expect("failed to initialize seccomp");

            if has_clone3() {
                return Err(TestCallbackError::Custom(
                    "clone3 is not blocked by seccomp".into(),
                ));
            }

            let pid = container_fork("test:child", || Ok(0)).map_err(|err| err.to_string())?;
            match waitpid(pid, None).expect("wait pid failed.") {
                WaitStatus::Exited(p, status) => {
                    assert_eq!(pid, p);
                    assert_eq!(status, 0);
                }
                _ => {
                    return Err(TestCallbackError::Custom(
                        "failed to wait on the child process".into(),
                    ));
                }
            };

            Ok(())
        })?;

        Ok(())
    }
}
