use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use libseccomp::scmp_compare::*;
use libseccomp::*;
use nix::errno::Errno;
use oci_spec::runtime::Arch;
use oci_spec::runtime::LinuxSeccomp;
use oci_spec::runtime::LinuxSeccompAction;
use oci_spec::runtime::LinuxSeccompOperator;
use std::ffi::CString;
use std::os::unix::io;

#[derive(Debug)]
struct Compare {
    // The zero-indexed index of the syscall arguement.
    arg: libc::c_uint,
    op: Option<scmp_compare>,
    datum_a: Option<scmp_datum_t>,
    datum_b: Option<scmp_datum_t>,
}

impl Compare {
    pub fn new(args: u32) -> Self {
        Compare {
            arg: args as libc::c_uint,
            op: None,
            datum_a: None,
            datum_b: None,
        }
    }

    pub fn op(mut self, op: scmp_compare) -> Self {
        self.op = Some(op);

        self
    }

    pub fn datum_a(mut self, datum: scmp_datum_t) -> Self {
        self.datum_a = Some(datum);

        self
    }

    pub fn datum_b(mut self, datum: scmp_datum_t) -> Self {
        self.datum_b = Some(datum);

        self
    }

    pub fn build(self) -> Result<scmp_arg_cmp> {
        if let Some((op, datum_a)) = self.op.zip(self.datum_a) {
            Ok(scmp_arg_cmp {
                arg: self.arg,
                op,
                datum_a,
                // datum_b is optional for a number of op, since these op only
                // requires one value. For example, the SCMP_OP_EQ or equal op
                // requires only one value. We set the datum_b to 0 in the case
                // that only one value is required.
                datum_b: self.datum_b.unwrap_or(0),
            })
        } else {
            bail!("op and datum_a is required: {:?}", self);
        }
    }
}

#[derive(Debug)]
struct Rule {
    action: u32,
    syscall_nr: i32,
    comparators: Vec<scmp_arg_cmp>,
}

impl Rule {
    pub fn new(action: u32, syscall_number: i32) -> Self {
        Rule {
            action,
            syscall_nr: syscall_number,
            comparators: vec![],
        }
    }

    pub fn add_comparator(&mut self, cmp: scmp_arg_cmp) {
        self.comparators.push(cmp);
    }
}

#[derive(Debug)]
struct FilterContext {
    ctx: scmp_filter_ctx,
}

impl FilterContext {
    pub fn default(default_action: u32) -> Result<FilterContext> {
        let filter_ctx = unsafe { seccomp_init(default_action) };
        if filter_ctx.is_null() {
            bail!("Failed to initialized seccomp profile")
        }

        Ok(FilterContext { ctx: filter_ctx })
    }

    pub fn add_rule(&mut self, rule: &Rule) -> Result<()> {
        let res = match rule.comparators.len() {
            0 => unsafe { seccomp_rule_add(self.ctx, rule.action, rule.syscall_nr, 0) },
            _ => unsafe {
                seccomp_rule_add_array(
                    self.ctx,
                    rule.action,
                    rule.syscall_nr,
                    rule.comparators.len() as u32,
                    rule.comparators.as_ptr(),
                )
            },
        };
        if res != 0 {
            bail!("Failed to add rule. Errno: {}, Rule: {:?}", res, rule);
        }

        Ok(())
    }

    pub fn add_arch(&mut self, arch: u32) -> Result<()> {
        let res = unsafe { seccomp_arch_add(self.ctx, arch) };
        if res != 0 && nix::Error::from_i32(res.abs()) != nix::Error::EEXIST {
            // The architecture already existed in the profile, so we can
            // safely ignore the error here. Otherwise, error out.
            bail!("Failed to add architecture {}. Errno: {}", arch, res);
        }

        Ok(())
    }

    pub fn load(&self) -> Result<()> {
        let res = unsafe { seccomp_load(self.ctx) };
        if res != 0 {
            bail!("Failed to load seccomp profile: {}", res);
        }

        Ok(())
    }

    pub fn notify_fd(&self) -> Result<Option<i32>> {
        let res = unsafe { seccomp_notify_fd(self.ctx) };
        if res > 0 {
            return Ok(Some(res));
        }

        // -1 indicates the notify fd is not set. This can happen if no seccomp
        // notify filter is set.
        if res == -1 {
            return Ok(None);
        }

        match nix::errno::from_i32(res.abs()) {
            Errno::EINVAL => {
                bail!("invalid seccomp context used to call notify fd");
            }
            Errno::EFAULT => {
                bail!("internal libseccomp fault; likely no seccomp filter is loaded");
            }
            Errno::EOPNOTSUPP => {
                bail!("seccomp notify filter not supported");
            }

            _ => {
                bail!("unknown error from return code: {}", res);
            }
        };
    }
}

fn translate_syscall(syscall_name: &str) -> Result<i32> {
    let c_syscall_name = CString::new(syscall_name)
        .with_context(|| format!("Failed to convert syscall {:?} to cstring", syscall_name))?;
    let res = unsafe { seccomp_syscall_resolve_name(c_syscall_name.as_ptr()) };
    if res == __NR_SCMP_ERROR {
        bail!("Failed to resolve syscall from name: {:?}", syscall_name);
    }

    Ok(res)
}

fn translate_action(action: LinuxSeccompAction, errno: Option<u32>) -> u32 {
    let errno = errno.unwrap_or(libc::EPERM as u32);
    match action {
        LinuxSeccompAction::ScmpActKill => SCMP_ACT_KILL,
        LinuxSeccompAction::ScmpActTrap => SCMP_ACT_TRAP,
        LinuxSeccompAction::ScmpActErrno => SCMP_ACT_ERRNO(errno),
        LinuxSeccompAction::ScmpActTrace => SCMP_ACT_TRACE(errno),
        LinuxSeccompAction::ScmpActAllow => SCMP_ACT_ALLOW,
        LinuxSeccompAction::ScmpActKillProcess => SCMP_ACT_KILL_PROCESS,
        LinuxSeccompAction::ScmpActNotify => SCMP_ACT_NOTIFY,
        LinuxSeccompAction::ScmpActLog => SCMP_ACT_LOG,
    }
}

fn translate_op(op: LinuxSeccompOperator) -> scmp_compare {
    match op {
        LinuxSeccompOperator::ScmpCmpNe => SCMP_CMP_NE,
        LinuxSeccompOperator::ScmpCmpLt => SCMP_CMP_LT,
        LinuxSeccompOperator::ScmpCmpLe => SCMP_CMP_LE,
        LinuxSeccompOperator::ScmpCmpEq => SCMP_CMP_EQ,
        LinuxSeccompOperator::ScmpCmpGe => SCMP_CMP_GE,
        LinuxSeccompOperator::ScmpCmpGt => SCMP_CMP_GT,
        LinuxSeccompOperator::ScmpCmpMaskedEq => SCMP_CMP_MASKED_EQ,
    }
}

fn translate_arch(arch: Arch) -> scmp_arch {
    match arch {
        Arch::ScmpArchNative => SCMP_ARCH_NATIVE,
        Arch::ScmpArchX86 => SCMP_ARCH_X86,
        Arch::ScmpArchX86_64 => SCMP_ARCH_X86_64,
        Arch::ScmpArchX32 => SCMP_ARCH_X32,
        Arch::ScmpArchArm => SCMP_ARCH_ARM,
        Arch::ScmpArchAarch64 => SCMP_ARCH_AARCH64,
        Arch::ScmpArchMips => SCMP_ARCH_MIPS,
        Arch::ScmpArchMips64 => SCMP_ARCH_MIPS64,
        Arch::ScmpArchMips64n32 => SCMP_ARCH_MIPS64N32,
        Arch::ScmpArchMipsel => SCMP_ARCH_MIPSEL,
        Arch::ScmpArchMipsel64 => SCMP_ARCH_MIPSEL64,
        Arch::ScmpArchMipsel64n32 => SCMP_ARCH_MIPSEL64N32,
        Arch::ScmpArchPpc => SCMP_ARCH_PPC,
        Arch::ScmpArchPpc64 => SCMP_ARCH_PPC64,
        Arch::ScmpArchPpc64le => SCMP_ARCH_PPC64LE,
        Arch::ScmpArchS390 => SCMP_ARCH_S390,
        Arch::ScmpArchS390x => SCMP_ARCH_S390X,
    }
}

fn check_seccomp(seccomp: &LinuxSeccomp) -> Result<()> {
    // We don't support notify as default action. After the seccomp filter is
    // created with notify, the container process will have to communicate the
    // returned fd to another process. Therefore, we need the write syscall or
    // otherwise, the write syscall will be block by the seccomp filter causing
    // the container process to hang. `runc` also disallow notify as default
    // action.
    // Note: read and close syscall are also used, because if we can
    // successfully write fd to another process, the other process can choose to
    // handle read/close syscall and allow read and close to proceed as
    // expected.
    if seccomp.default_action() == LinuxSeccompAction::ScmpActNotify {
        bail!("SCMP_ACT_NOTIFY cannot be used as default action");
    }

    if let Some(syscalls) = seccomp.syscalls() {
        for syscall in syscalls {
            if syscall.action() == LinuxSeccompAction::ScmpActNotify {
                for name in syscall.names() {
                    if name == "write" {
                        bail!("SCMP_ACT_NOTIFY cannot be used for the write syscall");
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn initialize_seccomp(seccomp: &LinuxSeccomp) -> Result<Option<io::RawFd>> {
    if seccomp.flags().is_some() {
        // runc did not support this, so let's skip it for now.
        bail!("seccomp flags are not yet supported");
    }

    check_seccomp(seccomp)?;

    let default_action = translate_action(seccomp.default_action(), seccomp.default_errno_ret());
    let mut ctx = FilterContext::default(default_action)?;

    if let Some(architectures) = seccomp.architectures() {
        for &arch in architectures {
            let arch_token = translate_arch(arch);
            ctx.add_arch(arch_token as u32)
                .context("failed to add arch to seccomp")?;
        }
    }

    // The SCMP_FLTATR_CTL_NNP controls if the seccomp load function will set
    // the new privilege bit automatically in prctl. Normally this is a good
    // thing, but for us we need better control. Based on the spec, if OCI
    // runtime spec doesn't set the no new privileges in Process, we should not
    // set it here.  If the seccomp load operation fails without enough
    // privilege, so be it. To prevent this automatic behavior, we unset the
    // value here.
    let ret = unsafe { seccomp_attr_set(ctx.ctx, scmp_filter_attr::SCMP_FLTATR_CTL_NNP, 0) };
    if ret != 0 {
        bail!(
            "failed to unset the no new privileges bit for seccomp: {}",
            ret
        );
    }

    if let Some(syscalls) = seccomp.syscalls() {
        for syscall in syscalls {
            let action = translate_action(syscall.action(), syscall.errno_ret());
            if action == default_action {
                // When the action is the same as the default action, the rule is redundent. We can
                // skip this here to avoid failing when we add the rules.
                log::warn!(
                    "Detect a seccomp action that is the same as the default action: {:?}",
                    syscall
                );
                continue;
            }

            for name in syscall.names() {
                let syscall_number = match translate_syscall(name) {
                    Ok(x) => x,
                    Err(_) => {
                        // If we failed to resolve the syscall by name, likely the kernel
                        // doeesn't support this syscall. So it is safe to skip...
                        log::warn!(
                            "Failed to resolve syscall, likely kernel doesn't support this. {:?}",
                            name
                        );
                        continue;
                    }
                };
                // Not clear why but if there are multiple arg attached to one
                // syscall rule, we have to add them seperatly. add_rule will
                // return EINVAL. runc does the same but doesn't explain why.
                match syscall.args() {
                    Some(args) => {
                        for arg in args {
                            let mut rule = Rule::new(action, syscall_number);
                            let cmp = Compare::new(arg.index() as u32)
                                .op(translate_op(arg.op()))
                                .datum_a(arg.value())
                                .datum_b(arg.value_two().unwrap_or(0))
                                .build()
                                .context("Failed to build a seccomp compare rule")?;
                            rule.add_comparator(cmp);
                            ctx.add_rule(&rule).with_context(|| {
                                format!(
                                    "failed to add seccomp rule: {:?}. Syscall: {:?}",
                                    &rule, name,
                                )
                            })?;
                        }
                    }
                    None => {
                        let rule = Rule::new(action, syscall_number);
                        ctx.add_rule(&rule).with_context(|| {
                            format!(
                                "failed to add seccomp rule: {:?}. Syscall: {:?}",
                                &rule, name,
                            )
                        })?;
                    }
                }
            }
        }
    }

    // In order to use the SECCOMP_SET_MODE_FILTER operation, either the calling
    // thread must have the CAP_SYS_ADMIN capability in its user namespace, or
    // the thread must already have the no_new_privs bit set.
    // Ref: https://man7.org/linux/man-pages/man2/seccomp.2.html
    ctx.load().context("failed to load seccomp context")?;

    let fd = if is_notify(seccomp) {
        ctx.notify_fd().context("failed to get seccomp notify fd")?
    } else {
        None
    };

    Ok(fd)
}

pub fn is_notify(seccomp: &LinuxSeccomp) -> bool {
    seccomp
        .syscalls()
        .iter()
        .flatten()
        .any(|syscall| syscall.action() == LinuxSeccompAction::ScmpActNotify)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils;
    use anyhow::Result;
    use oci_spec::runtime::Arch;
    use oci_spec::runtime::{LinuxSeccompBuilder, LinuxSyscallBuilder};
    use serial_test::serial;
    use std::path;

    #[test]
    #[serial]
    fn test_basic() -> Result<()> {
        // Note: seccomp profile is really hard to write unit test for. First,
        // we can't really test default error or kill action, since rust test
        // actually relies on certain syscalls. Second, some of the syscall will
        // not return errorno. These syscalls will just send an abort signal or
        // even just segfaults.  Here we choose to use `getcwd` syscall for
        // testing, since it will correctly return an error under seccomp rule.
        // This is more of a sanity check.

        // Here, we choose an error that getcwd call would never return on its own, so
        // we can make sure that getcwd failed because of seccomp rule.
        let expect_error = libc::EAGAIN;

        let syscall = LinuxSyscallBuilder::default()
            .names(vec![String::from("getcwd")])
            .action(LinuxSeccompAction::ScmpActErrno)
            .errno_ret(expect_error as u32)
            .build()?;
        let seccomp_profile = LinuxSeccompBuilder::default()
            .default_action(LinuxSeccompAction::ScmpActAllow)
            .architectures(vec![Arch::ScmpArchNative])
            .syscalls(vec![syscall])
            .build()?;

        test_utils::test_in_child_process(|| {
            let _ = prctl::set_no_new_privileges(true);
            initialize_seccomp(&seccomp_profile)?;
            let ret = nix::unistd::getcwd();
            if ret.is_ok() {
                bail!("getcwd didn't error out as seccomp profile specified");
            }

            if let Some(errno) = ret.err() {
                if errno != nix::errno::from_i32(expect_error) {
                    bail!(
                        "getcwd failed but we didn't get the expected error from seccomp profile: {}", errno
                    );
                }
            }

            Ok(())
        })?;

        Ok(())
    }

    #[test]
    #[serial]
    fn test_moby() -> Result<()> {
        let fixture_path =
            path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/seccomp/fixture/config.json");
        let spec = oci_spec::runtime::Spec::load(fixture_path)
            .context("Failed to load test spec for seccomp")?;

        // We know linux and seccomp exist, so let's just unwrap.
        let seccomp_profile = spec.linux().as_ref().unwrap().seccomp().as_ref().unwrap();
        test_utils::test_in_child_process(|| {
            let _ = prctl::set_no_new_privileges(true);
            initialize_seccomp(seccomp_profile)?;

            Ok(())
        })?;

        Ok(())
    }

    #[test]
    #[serial]
    fn test_seccomp_notify() -> Result<()> {
        let syscall = LinuxSyscallBuilder::default()
            .names(vec![String::from("getcwd")])
            .action(LinuxSeccompAction::ScmpActNotify)
            .build()?;
        let seccomp_profile = LinuxSeccompBuilder::default()
            .default_action(LinuxSeccompAction::ScmpActAllow)
            .architectures(vec![Arch::ScmpArchNative])
            .syscalls(vec![syscall])
            .build()?;
        test_utils::test_in_child_process(|| {
            let _ = prctl::set_no_new_privileges(true);
            let fd = initialize_seccomp(&seccomp_profile)?;
            if fd.is_none() {
                bail!("failed to get a seccomp notify fd with notify seccomp profile");
            }

            Ok(())
        })?;

        Ok(())
    }
}
