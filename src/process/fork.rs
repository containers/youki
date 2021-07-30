use anyhow::Context;
use anyhow::Result;
use libc::c_int;
use libc::c_void;
use nix::errno::Errno;
use nix::sched;
use nix::sys;
use nix::sys::mman;
use nix::unistd::Pid;
use std::mem;
use std::ptr;

/// clone uses syscall clone(2) to create a new process for the container init
/// process. Using clone syscall gives us better control over how to can create
/// the new container process, where we can enter into namespaces directly instead
/// of using unshare and fork. This call will only create one new process, instead
/// of two using fork.
pub fn clone(mut cb: sched::CloneCb, clone_flags: sched::CloneFlags) -> Result<Pid> {
    extern "C" fn callback(data: *mut sched::CloneCb) -> c_int {
        let cb: &mut sched::CloneCb = unsafe { &mut *data };
        (*cb)() as c_int
    }

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

    // Adds SIGCHLD flag to mimic the same behavior as fork.
    let signal = sys::signal::Signal::SIGCHLD;
    let combined = clone_flags.bits() | signal as c_int;
    let res = unsafe {
        // Consistant with how pthread_create sets up the stack, we create a
        // guard page of 1 page, to protect the child stack collision. Note, for
        // clone call, the child stack will grow downward, so the bottom of the
        // child stack is in the beginning.
        mman::mprotect(child_stack, page_size, mman::ProtFlags::PROT_NONE)
            .with_context(|| "Failed to create guard page")?;

        // Since the child stack for clone grows downward, we need to pass in
        // the top of the stack address.
        let child_stack_top = child_stack.add(default_stack_size);

        // Using the clone syscall is one of the rare cases where we don't want
        // rust to manage the child stack memory.  Instead, we want to use
        // c_void directly here. The nix::sched::clone wrapper doesn't provide
        // the right interface and its interface can't be changed. Therefore,
        // here we are using libc::clone syscall directly for better control.
        // The child stack will be cleaned when exec is called or the child
        // process terminates.
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
