use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use nix::errno::Errno;
use nix::sched;
use nix::sys;
use nix::sys::mman;
use nix::unistd::Pid;
use std::ptr;

// The clone callback is used in clone call. It is a boxed closure and it needs
// to trasfer the ownership of related memory to the new process.
type CloneCb = Box<dyn FnOnce() -> isize + Send>;

/// clone uses syscall clone(2) to create a new process for the container init
/// process. Using clone syscall gives us better control over how to can create
/// the new container process, where we can enter into namespaces directly instead
/// of using unshare and fork. This call will only create one new process, instead
/// of two using fork.
pub fn clone(cb: CloneCb, clone_flags: sched::CloneFlags) -> Result<Pid> {
    // Use sysconf to find the page size. If there is an error, we assume
    // the default 4K page size.
    let page_size: usize = unsafe {
        match libc::sysconf(libc::_SC_PAGE_SIZE) {
            -1 => 4 * 1024, // default to 4K page size
            x => x as usize,
        }
    };

    // Find out the default stack max size through getrlimit.
    let mut rlimit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    unsafe { Errno::result(libc::getrlimit(libc::RLIMIT_STACK, &mut rlimit))? };
    let default_stack_size = rlimit.rlim_cur as usize;

    // Using the clone syscall requires us to create the stack space for the
    // child process instead of taken cared for us like fork call. We use mmap
    // here to create the stack.  Instead of guessing how much space the child
    // process needs, we allocate through mmap to the system default limit,
    // which is 8MB on most of the linux system today. This is OK since mmap
    // will only researve the address space upfront, instead of allocating
    // physical memory upfront.  The stack will grow as needed, up to the size
    // researved, so no wasted memory here. Lastly, the child stack only needs
    // to support the container init process set up code in Youki. When Youki
    // calls exec into the container payload, exec will reset the stack.  Note,
    // do not use MAP_GROWSDOWN since it is not well supported.
    // Ref: https://man7.org/linux/man-pages/man2/mmap.2.html
    let child_stack = unsafe {
        mman::mmap(
            ptr::null_mut(),
            default_stack_size,
            mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
            mman::MapFlags::MAP_PRIVATE | mman::MapFlags::MAP_ANONYMOUS | mman::MapFlags::MAP_STACK,
            -1,
            0,
        )?
    };
    // Consistant with how pthread_create sets up the stack, we create a
    // guard page of 1 page, to protect the child stack collision. Note, for
    // clone call, the child stack will grow downward, so the bottom of the
    // child stack is in the beginning.
    unsafe {
        mman::mprotect(child_stack, page_size, mman::ProtFlags::PROT_NONE)
            .with_context(|| "Failed to create guard page")?
    };

    // Since the child stack for clone grows downward, we need to pass in
    // the top of the stack address.
    let child_stack_top = unsafe { child_stack.add(default_stack_size) };

    // Adds SIGCHLD flag to mimic the same behavior as fork.
    let signal = sys::signal::Signal::SIGCHLD;
    let combined = clone_flags.bits() | signal as libc::c_int;

    // We are passing the boxed closure "cb" into the clone function as the a
    // function pointer in C. The box closure in Rust is both a function pointer
    // and a struct. However, when casting the box closure into libc::c_void,
    // the function pointer will be lost. Therefore, to work around the issue,
    // we double box the closure. This is consistant with how std::unix::thread
    // handles the closure.
    // Ref: https://github.com/rust-lang/rust/blob/master/library/std/src/sys/unix/thread.rs
    let data = Box::into_raw(Box::new(cb));
    // The main is a wrapper function passed into clone call below. The "data"
    // arg is actually a raw pointer to a Box closure. so here, we re-box the
    // pointer back into a box closure so the main takes ownership of the
    // memory. Then we can call the closure passed in.
    extern "C" fn main(data: *mut libc::c_void) -> libc::c_int {
        unsafe { Box::from_raw(data as *mut CloneCb)() as i32 }
    }

    // The nix::sched::clone wrapper doesn't provide the right interface.  Using
    // the clone syscall is one of the rare cases where we don't want rust to
    // manage the child stack memory. Instead, we want to use c_void directly
    // here.  Therefore, here we are using libc::clone syscall directly for
    // better control.  The child stack will be cleaned when exec is called or
    // the child process terminates. The nix wrapper also does not treat the
    // closure memory correctly. The wrapper implementation fails to pass the
    // right ownership to the new child process.
    // Ref: https://github.com/nix-rust/nix/issues/919
    // Ref: https://github.com/nix-rust/nix/pull/920
    let res = unsafe { libc::clone(main, child_stack_top, combined, data as *mut libc::c_void) };
    match res {
        -1 => {
            // Since the clone call failed, the closure passed in didn't get
            // consumed. To complete the circle, we can safely box up the
            // closure again and let rust manage this memory for us.
            unsafe { drop(Box::from_raw(data)) };
            bail!(
                "Failed clone to create new process: {:?}",
                Errno::result(res)
            )
        }
        pid => Ok(Pid::from_raw(pid)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use nix::sys::wait;
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
            Box::new(move || {
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

    fn clone_closure_ownership_test_payload() -> super::CloneCb {
        // The vec should not be deallocated after this function returns. The
        // ownership should correctly transfer to the closure returned, to be
        // passed to the clone and new child process.
        let numbers: Vec<i32> = (0..101).into_iter().collect();
        Box::new(move || {
            assert_eq!(numbers.iter().sum::<i32>(), 5050);
            0
        })
    }

    #[test]
    fn test_clone_closure_ownership() -> Result<()> {
        let flags = sched::CloneFlags::empty();

        let pid = super::clone(clone_closure_ownership_test_payload(), flags)?;
        let exit_status =
            wait::waitpid(pid, Some(wait::WaitPidFlag::__WALL)).expect("Waiting for child");
        assert_eq!(exit_status, wait::WaitStatus::Exited(pid, 0));

        Ok(())
    }
}
