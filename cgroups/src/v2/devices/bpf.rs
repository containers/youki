use anyhow::{bail, Result};
use std::os::unix::io::RawFd;

// FIXME: add tests

pub fn prog_load(license: &str, insns: &[u8]) -> Result<RawFd> {
    let insns_cnt = insns.len() / std::mem::size_of::<libbpf_sys::bpf_insn>();
    let insns = insns as *const _ as *const libbpf_sys::bpf_insn;

    let prog_fd = unsafe {
        libbpf_sys::bpf_load_program(
            libbpf_sys::BPF_PROG_TYPE_CGROUP_DEVICE,
            insns,
            insns_cnt as u64,
            license as *const _ as *const i8,
            0,
            0 as *mut i8,
            0,
        )
    };

    if prog_fd < 0 {
        return Err(errno::errno().into());
    }
    Ok(prog_fd)
}

pub struct ProgramInfo {
    pub id: u32,
    pub fd: i32,
}

pub fn prog_query(cgroup_fd: RawFd) -> Result<Vec<ProgramInfo>> {
    let mut prog_ids: Vec<u32> = vec![0_u32; 64];
    let mut attach_flags = 0_u32;
    for _ in 0..10 {
        let mut prog_cnt = prog_ids.len() as u32;
        let ret = unsafe {
            libbpf_sys::bpf_prog_query(
                cgroup_fd,
                libbpf_sys::BPF_CGROUP_DEVICE,
                0,
                &mut attach_flags,
                &prog_ids[0] as *const u32 as *mut u32,
                &mut prog_cnt,
            )
        };
        if ret != 0 {
            let err = errno::errno();
            if err.0 == libc::ENOSPC {
                assert!(prog_cnt as usize > prog_ids.len());

                // allocate more space and try again
                prog_ids.resize(prog_cnt as usize, 0);
                continue;
            }

            return Err(err.into());
        }

        prog_ids.resize(prog_cnt as usize, 0);
        break;
    }

    let mut prog_fds = Vec::with_capacity(prog_ids.len());
    for prog_id in &prog_ids {
        let prog_fd = unsafe { libbpf_sys::bpf_prog_get_fd_by_id(*prog_id) };
        if prog_fd < 0 {
            log::debug!("bpf_prog_get_fd_by_id failed: {}", errno::errno());
            continue;
        }
        prog_fds.push(ProgramInfo {
            id: *prog_id,
            fd: prog_fd,
        });
    }
    Ok(prog_fds)
}

pub fn prog_detach2(prog_fd: RawFd, cgroup_fd: RawFd) -> Result<()> {
    let ret =
        unsafe { libbpf_sys::bpf_prog_detach2(prog_fd, cgroup_fd, libbpf_sys::BPF_CGROUP_DEVICE) };
    if ret != 0 {
        return Err(errno::errno().into());
    }
    Ok(())
}

pub fn prog_attach(prog_fd: RawFd, cgroup_fd: RawFd) -> Result<()> {
    let ret = unsafe {
        libbpf_sys::bpf_prog_attach(
            prog_fd,
            cgroup_fd,
            libbpf_sys::BPF_CGROUP_DEVICE,
            libbpf_sys::BPF_F_ALLOW_MULTI,
        )
    };

    if ret != 0 {
        return Err(errno::errno().into());
    }
    Ok(())
}

pub fn bump_memlock_rlimit() -> Result<()> {
    let rlimit = libc::rlimit {
        rlim_cur: 128 << 20,
        rlim_max: 128 << 20,
    };

    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlimit) } != 0 {
        bail!("Failed to increase rlimit");
    }

    Ok(())
}
