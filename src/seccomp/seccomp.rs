#[macro_use]
use std::mem;

// 3. Maps Actions
// 4. Define allowed system calls

use bindings::{
  SECCOMP_RET_LOG, 
  SYSCALL_MAP,
};
use nix::errno::Errno;

const SECCOMP_RET_KILL: u32 = 0;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;

const BPF_LD: u16 = 0x00;
const BPF_JMP: u16 = 0x05;
const BPF_RET: u16 = 0x06;
const BPF_W: u16 = 0;
const BPF_ABS: u16 = 0x20;
const BPF_JEQ: u16 = 0x10;
const BPF_JSET: u16 = 0x40;
const BPF_K: u16 = 0x00;

const EM_386: u32 = 3;
const EM_PPC: u32 = 20;
const EM_PPC64: u32 = 21;
const EM_ARM: u32 = 40;
const EM_X86_64: u32 = 62;
const EM_AARCH64: u32 = 183;

#[repr(C)]
#[derive(Copy, Clone)]
struct sock_filter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct sock_fprog {
    len: c_ushort,
    filter: *const sock_filter,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct seccomp_data {
    nr: i32,
    arch: u32,
    instruction_pointer: u64,
    args: u64
}

/// The architecture number for x86.
#[cfg(target_arch="x86")]
const ARCH_NR: u32 = AUDIT_ARCH_X86;
/// The architecture number for x86-64.
#[cfg(target_arch="x86_64")]
const ARCH_NR: u32 = AUDIT_ARCH_X86_64;
/// The architecture number for ARM.
#[cfg(target_arch="arm")]
const ARCH_NR: u32 = AUDIT_ARCH_ARM;
/// The architecture number for ARM 64-bit.
#[cfg(target_arch="aarch64")]
const ARCH_NR: u32 = AUDIT_ARCH_AARCH64;
#[cfg(target_arch="powerpc")]
const ARCH_NR: u32 = AUDIT_ARCH_PPC;
#[cfg(all(target_arch="powerpc64", target_endian="big"))]
const ARCH_NR: u32 = AUDIT_ARCH_PPC64;
#[cfg(all(target_arch="powerpc64", target_endian="little"))]
const ARCH_NR: u32 = AUDIT_ARCH_PPC64LE;

#[cfg(all(target_arch = "aarch64"))]
const REQUIRED_SYSCALLS: &[u32] = &[
  // TODO
];

#[cfg(all(target_arch = "x86_64"))]
const REQUIRED_SYSCALLS: [u32; 21] = [
  libc::SYS_brk as u32,
  libc::SYS_close as u32,
  libc::SYS_exit as u32,
  libc::SYS_exit_group as u32,
  libc::SYS_futex as u32,
  libc::SYS_getrandom as u32,
  libc::SYS_getuid as u32,
  libc::SYS_mmap as u32,
  libc::SYS_mprotect as u32,
  libc::SYS_munmap as u32,
  libc::SYS_poll as u32,
  libc::SYS_read as u32,
  libc::SYS_recvfrom as u32,
  libc::SYS_recvmsg as u32,
  libc::SYS_rt_sigreturn as u32,
  libc::SYS_sched_getaffinity as u32,
  libc::SYS_sendmmsg as u32,
  libc::SYS_sendto as u32,
  libc::SYS_set_robust_list as u32,
  libc::SYS_sigaltstack as u32,
  libc::SYS_write as u32,
];

pub struct Filter {
  allowlist: Vec<sock_filter>
}

impl Filter {
  const EVAL_NEXT: u8 = 0;
  const SKIP_NEXT: u8 = 1;

  /// Create a new secommp filter
  pub fn new() -> Self {
    let mut filter = Filter {
      allowlist: Vec::new(),
      log_only: false,
    };

    // This ensures that a malicious process cannot configure a bad seccomp-BPF program and
    // then execve to a set-uid program, potentially permitting privilege escalation.
    use nix::libc::PR_SET_NO_NEW_PRIVS;
    let result = unsafe { nix::libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    Errno::result(result)
    .map(drop)
    .expect("Failed to set seccomp filter")

    // Load architecture into accumulator
    filter.allowlist.push(bpf_stmt(
      BPF_LD | BPF_W | BPF_ABS,
      mem::offset_of!(seccomp_data, arch) as u32,
    ));

    // Kill process if architecture does not match
    filter.allowlist.push(bpf_jump(
      BPF_JMP | BPF_JEQ | BPF_K,
      AUDIT_ARCH,
      Filter::SKIP_NEXT,
      Filter::EVAL_NEXT,
    ));
    filter.allowlist.push(bpf_ret(SECCOMP_RET_KILL));

    // Load system call number into accumulator for subsequent filtering
    filter.allowlist.push(bpf_stmt(
      BPF_LD | BPF_W | BPF_ABS,
      mem::offset_of!(seccomp_data, nr) as u32,
    ));

    // Add default allowlist for architecture
    for syscall in REQUIRED_SYSCALLS {
      filter = filter.allow_syscall_nr(*syscall as u32);
    }
    filter
  }

  pub fn allow_syscall_nr(mut self, nr: u32) -> Filter {
    // If syscall matches return 'allow' directly. If not, skip return instruction and go to next check.
    self.allowlist.push(bpf_jump(
      BPF_JMP | BPF_JEQ | BPF_K,
      nr,
      Filter::EVAL_NEXT,
      Filter::SKIP_NEXT,
    ));
    self.allowlist.push(bpf_ret(SECCOMP_RET_ALLOW));
    self
  }

  /// Add syscall name to whitelist
  pub fn allow_syscall_name(self, name: &str) -> Filter {
    let syscall_nr = translate_syscall(name).expect("Failed to translate syscall");
    self.allow_syscall_nr(syscall_nr)
  }

  /// Log syscall violations only
  #[allow(unused)]
  pub fn log_only(mut self) -> Filter {
    self.log_only = true;
    self
  }

  /// Apply seccomp rules
  pub fn apply(mut self) {
    // use unix const
    use nix::libc::PR_SET_SECCOMP;
    use nix::libc::SECCOMP_MODE_FILTER;

    let sf_prog = sock_fprog {
      len: self.allowlist.len() as u16,
      filter: self.allowlist.as_mut_ptr(),
    };
    let sf_prog_ptr = &sf_prog as *const sock_fprog;
    let result = unsafe { nix::libc::prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, sf_prog_ptr) };
    Errno::result(result)
      .map(drop)
      .expect("Failed to set seccomp filter")
  }
}

/// Get number of systemcall for a name
fn translate_syscall(name: &str) -> Option<u32> {
  SYSCALL_MAP.get(name).cloned()
}

// https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/filter.h
fn bpf_stmt(code: u32, k: u32) -> sock_filter {
  sock_filter {
    code: code as u16,
    k,
    jt: 0,
    jf: 0,
  }
}

fn bpf_jump(code: u32, k: u32, jt: u8, jf: u8) -> sock_filter {
  sock_filter {
    code: code as u16,
    k,
    jt,
    jf,
  }
}

fn bpf_ret(k: u32) -> sock_filter {
  bpf_stmt(BPF_RET | BPF_K, k)
}
