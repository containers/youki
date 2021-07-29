use anyhow::Result;
use libc::c_int;
use libc::c_void;
use nix::errno::Errno;
use nix::sched;
use nix::unistd::Pid;
use std::mem;

pub fn clone(mut cb: sched::CloneCb, clone_flags: sched::CloneFlags) -> Result<Pid> {
    extern "C" fn callback(data: *mut sched::CloneCb) -> c_int {
        let cb: &mut sched::CloneCb = unsafe { &mut *data };
        (*cb)() as c_int
    }

    let child_stack_top = unsafe {
        let page_size: usize = match libc::sysconf(libc::_SC_PAGE_SIZE) {
            -1 => 4 * 1024, // default to 4K page size
            x => x as usize,
        };

        let mut rlimit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        Errno::result(libc::getrlimit(libc::RLIMIT_STACK, &mut rlimit))?;
        let default_stack_size = rlimit.rlim_cur as usize;

        let child_stack = libc::mmap(
            libc::PT_NULL as *mut c_void,
            default_stack_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_STACK,
            -1,
            0,
        );
        Errno::result(libc::mprotect(child_stack, page_size, libc::PROT_NONE))?;
        let child_stack_top = child_stack.add(default_stack_size);

        child_stack_top
    };

    let res = unsafe {
        let signal = nix::sys::signal::Signal::SIGCHLD;
        let combined = clone_flags.bits() | signal as c_int;
        libc::clone(
            mem::transmute(callback as extern "C" fn(*mut Box<dyn FnMut() -> isize>) -> i32),
            child_stack_top,
            combined,
            &mut cb as *mut _ as *mut c_void,
        )
    };
    let pid = Errno::result(res).map(Pid::from_raw)?;

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
            // In a new pid namespace, pid of this process should be 1
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

    #[test]
    fn test_clone_stack_allocation() -> Result<()> {
        let flags = sched::CloneFlags::empty();
        let pid = super::clone(
            Box::new(|| {
                let mut array_on_stack = [0u8; 4096];
                array_on_stack.iter_mut().for_each(|x| *x = 0);

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
