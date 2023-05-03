use super::args::{ContainerArgs, ContainerType};
use crate::apparmor;
use crate::syscall::{Syscall, SyscallError};
use crate::{
    capabilities, hooks, namespaces::Namespaces, process::channel, rootfs::RootFS,
    rootless::Rootless, tty, utils,
};
use anyhow::{bail, Context, Ok, Result};
use nix::mount::MsFlags;
use nix::sched::CloneFlags;
use nix::sys::stat::Mode;
use nix::unistd::setsid;
use nix::unistd::{self, Gid, Uid};
use oci_spec::runtime::{LinuxNamespaceType, Spec, User};
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "libseccomp")]
use crate::seccomp;

#[cfg(not(feature = "libseccomp"))]
use log::warn;

fn sysctl(kernel_params: &HashMap<String, String>) -> Result<()> {
    let sys = PathBuf::from("/proc/sys");
    for (kernel_param, value) in kernel_params {
        let path = sys.join(kernel_param.replace('.', "/"));
        log::debug!(
            "apply value {} to kernel parameter {}.",
            value,
            kernel_param
        );
        fs::write(path, value.as_bytes())
            .with_context(|| format!("failed to set sysctl {kernel_param}={value}"))?;
    }

    Ok(())
}

// make a read only path
// The first time we bind mount, other flags are ignored,
// so we need to mount it once and then remount it with the necessary flags specified.
// https://man7.org/linux/man-pages/man2/mount.2.html
fn readonly_path(path: &Path, syscall: &dyn Syscall) -> Result<()> {
    if let Err(err) = syscall.mount(
        Some(path),
        path,
        None,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None,
    ) {
        match err {
            SyscallError::MountFailed { errno, .. } => {
                // ignore error if path is not exist.
                if matches!(errno, nix::errno::Errno::ENOENT) {
                    return Ok(());
                }
            }
            _ => bail!(err),
        }
    }

    syscall.mount(
        Some(path),
        path,
        None,
        MsFlags::MS_NOSUID
            | MsFlags::MS_NODEV
            | MsFlags::MS_NOEXEC
            | MsFlags::MS_BIND
            | MsFlags::MS_REMOUNT
            | MsFlags::MS_RDONLY,
        None,
    )?;

    log::debug!("readonly path {:?} mounted", path);
    Ok(())
}

// For files, bind mounts /dev/null over the top of the specified path.
// For directories, mounts read-only tmpfs over the top of the specified path.
fn masked_path(path: &Path, mount_label: &Option<String>, syscall: &dyn Syscall) -> Result<()> {
    if let Err(err) = syscall.mount(
        Some(Path::new("/dev/null")),
        path,
        None,
        MsFlags::MS_BIND,
        None,
    ) {
        match err {
            SyscallError::MountFailed { errno, .. } => match errno {
                nix::errno::Errno::ENOENT => {
                    log::warn!("masked path {:?} not exist", path);
                }
                nix::errno::Errno::ENOTDIR => {
                    let label = match mount_label {
                        Some(l) => format!("context=\"{l}\""),
                        None => "".to_string(),
                    };
                    syscall.mount(
                        Some(Path::new("tmpfs")),
                        path,
                        Some("tmpfs"),
                        MsFlags::MS_RDONLY,
                        Some(label.as_str()),
                    )?;
                }
                _ => {
                    bail!(err)
                }
            },
            _ => bail!(err),
        }
    }

    Ok(())
}

// Enter into rest of namespace. Note, we already entered into user and pid
// namespace. We also have to enter into mount namespace last since
// namespace may be bind to /proc path. The /proc path will need to be
// accessed before pivot_root.
fn apply_rest_namespaces(
    namespaces: &Namespaces,
    spec: &Spec,
    syscall: &dyn Syscall,
) -> Result<()> {
    namespaces
        .apply_namespaces(|ns_type| -> bool {
            ns_type != CloneFlags::CLONE_NEWUSER && ns_type != CloneFlags::CLONE_NEWPID
        })
        .with_context(|| "failed to apply namespaces")?;

    // Only set the host name if entering into a new uts namespace
    if let Some(uts_namespace) = namespaces.get(LinuxNamespaceType::Uts) {
        if uts_namespace.path().is_none() {
            if let Some(hostname) = spec.hostname() {
                syscall.set_hostname(hostname)?;
            }

            if let Some(domainname) = spec.domainname() {
                syscall.set_domainname(domainname)?;
            }
        }
    }
    Ok(())
}

fn reopen_dev_null() -> Result<()> {
    // At this point we should be inside of the container and now
    // we can re-open /dev/null if it is in use to the /dev/null
    // in the container.

    let dev_null = fs::File::open("/dev/null")?;
    let dev_null_fstat_info = nix::sys::stat::fstat(dev_null.as_raw_fd())?;

    // Check if stdin, stdout or stderr point to /dev/null
    for fd in 0..3 {
        let fstat_info = nix::sys::stat::fstat(fd)?;

        if dev_null_fstat_info.st_rdev == fstat_info.st_rdev {
            // This FD points to /dev/null outside of the container.
            // Let's point to /dev/null inside of the container.
            nix::unistd::dup2(dev_null.as_raw_fd(), fd)?;
        }
    }
    Ok(())
}

#[allow(unused_variables)]
pub fn container_init_process(
    args: &ContainerArgs,
    main_sender: &mut channel::MainSender,
    init_receiver: &mut channel::InitReceiver,
) -> Result<()> {
    let syscall = args.syscall;
    let spec = args.spec;
    let linux = spec.linux().as_ref().context("no linux in spec")?;
    let proc = spec.process().as_ref().context("no process in spec")?;
    let mut envs: Vec<String> = proc.env().as_ref().unwrap_or(&vec![]).clone();
    let rootfs_path = args.rootfs;
    let hooks = spec.hooks().as_ref();
    let container = args.container.as_ref();
    let namespaces = Namespaces::from(linux.namespaces().as_ref());

    setsid().context("failed to create session")?;
    // set up tty if specified
    if let Some(csocketfd) = args.console_socket {
        tty::setup_console(&csocketfd).with_context(|| "failed to set up tty")?;
    }

    apply_rest_namespaces(&namespaces, spec, syscall)?;

    if let Some(true) = proc.no_new_privileges() {
        let _ = prctl::set_no_new_privileges(true);
    }

    if matches!(args.container_type, ContainerType::InitContainer) {
        // create_container hook needs to be called after the namespace setup, but
        // before pivot_root is called. This runs in the container namespaces.
        if let Some(hooks) = hooks {
            hooks::run_hooks(hooks.create_container().as_ref(), container)
                .context("Failed to run create container hooks")?;
        }

        let bind_service = namespaces.get(LinuxNamespaceType::User).is_some();
        let rootfs = RootFS::new();
        rootfs
            .prepare_rootfs(
                spec,
                rootfs_path,
                bind_service,
                namespaces.get(LinuxNamespaceType::Cgroup).is_some(),
            )
            .with_context(|| "Failed to prepare rootfs")?;

        // Entering into the rootfs jail. If mount namespace is specified, then
        // we use pivot_root, but if we are on the host mount namespace, we will
        // use simple chroot. Scary things will happen if you try to pivot_root
        // in the host mount namespace...
        if namespaces.get(LinuxNamespaceType::Mount).is_some() {
            // change the root of filesystem of the process to the rootfs
            syscall
                .pivot_rootfs(rootfs_path)
                .with_context(|| format!("failed to pivot root to {rootfs_path:?}"))?;
        } else {
            syscall
                .chroot(rootfs_path)
                .with_context(|| format!("failed to chroot to {rootfs_path:?}"))?;
        }

        rootfs
            .adjust_root_mount_propagation(linux)
            .context("failed to set propagation type of root mount")?;

        reopen_dev_null()?;

        if let Some(kernel_params) = linux.sysctl() {
            sysctl(kernel_params)
                .with_context(|| format!("failed to sysctl: {kernel_params:?}"))?;
        }
    }

    if let Some(profile) = proc.apparmor_profile() {
        apparmor::apply_profile(profile)
            .with_context(|| format!("failed to apply apparmor profile {profile}"))?;
    }

    if let Some(true) = spec.root().as_ref().map(|r| r.readonly().unwrap_or(false)) {
        syscall.mount(
            None,
            Path::new("/"),
            None,
            MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT | MsFlags::MS_BIND,
            None,
        )?
    }

    if let Some(umask) = proc.user().umask() {
        if let Some(mode) = Mode::from_bits(umask) {
            nix::sys::stat::umask(mode);
        } else {
            bail!("invalid umask {}", umask);
        }
    }

    if let Some(paths) = linux.readonly_paths() {
        // mount readonly path
        for path in paths {
            readonly_path(Path::new(path), syscall)
                .with_context(|| format!("failed to set read only path {path:?}"))?;
        }
    }

    if let Some(paths) = linux.masked_paths() {
        // mount masked path
        for path in paths {
            masked_path(Path::new(path), linux.mount_label(), syscall)
                .with_context(|| format!("failed to set masked path {path:?}"))?;
        }
    }

    let cwd = format!("{}", proc.cwd().display());
    let do_chdir = if cwd.is_empty() {
        false
    } else {
        // This chdir must run before setting up the user.
        // This may allow the user running youki to access directories
        // that the container user cannot access.
        match unistd::chdir(proc.cwd()) {
            std::result::Result::Ok(_) => false,
            Err(nix::Error::EPERM) => true,
            Err(e) => bail!("failed to chdir: {}", e),
        }
    };

    set_supplementary_gids(proc.user(), args.rootless, syscall)
        .context("failed to set supplementary gids")?;

    syscall
        .set_id(
            Uid::from_raw(proc.user().uid()),
            Gid::from_raw(proc.user().gid()),
        )
        .context("failed to configure uid and gid")?;

    // Take care of LISTEN_FDS used for systemd-active-socket. If the value is
    // not 0, then we have to preserve those fds as well, and set up the correct
    // environment variables.
    let preserve_fds: i32 = match env::var("LISTEN_FDS") {
        std::result::Result::Ok(listen_fds_str) => {
            let listen_fds = match listen_fds_str.parse::<i32>() {
                std::result::Result::Ok(v) => v,
                Err(error) => {
                    log::warn!(
                        "LISTEN_FDS entered is not a fd. Ignore the value. {:?}",
                        error
                    );

                    0
                }
            };

            // The LISTEN_FDS will have to be passed to container init process.
            // The LISTEN_PID will be set to PID 1. Based on the spec, if
            // LISTEN_FDS is 0, the variable should be unset, so we just ignore
            // it here, if it is 0.
            if listen_fds > 0 {
                envs.append(&mut vec![
                    format!("LISTEN_FDS={listen_fds}"),
                    "LISTEN_PID=1".to_string(),
                ]);
            }

            args.preserve_fds + listen_fds
        }
        Err(env::VarError::NotPresent) => args.preserve_fds,
        Err(env::VarError::NotUnicode(value)) => {
            log::warn!(
                "LISTEN_FDS entered is malformed: {:?}. Ignore the value.",
                &value
            );
            args.preserve_fds
        }
    };

    // Cleanup any extra file descriptors, so the new container process will not
    // leak a file descriptor from before execve gets executed. The first 3 fd will
    // stay open: stdio, stdout, and stderr. We would further preserve the next
    // "preserve_fds" number of fds. Set the rest of fd with CLOEXEC flag, so they
    // will be closed after execve into the container payload. We can't close the
    // fds immediately since we at least still need it for the pipe used to wait on
    // starting the container.
    syscall
        .close_range(preserve_fds)
        .with_context(|| "failed to clean up extra fds")?;

    // Without no new privileges, seccomp is a privileged operation. We have to
    // do this before dropping capabilities. Otherwise, we should do it later,
    // as close to exec as possible.
    #[cfg(feature = "libseccomp")]
    if let Some(seccomp) = linux.seccomp() {
        if proc.no_new_privileges().is_none() {
            let notify_fd =
                seccomp::initialize_seccomp(seccomp).context("failed to execute seccomp")?;
            sync_seccomp(notify_fd, main_sender, init_receiver)
                .context("failed to sync seccomp")?;
        }
    }
    #[cfg(not(feature = "libseccomp"))]
    if proc.no_new_privileges().is_none() {
        warn!("seccomp not available, unable to enforce no_new_privileges!")
    }

    capabilities::reset_effective(syscall).context("Failed to reset effective capabilities")?;
    if let Some(caps) = proc.capabilities() {
        capabilities::drop_privileges(caps, syscall).context("Failed to drop capabilities")?;
    }

    // Change directory to process.cwd if process.cwd is not empty
    if do_chdir {
        unistd::chdir(proc.cwd()).with_context(|| format!("failed to chdir {:?}", proc.cwd()))?;
    }

    // add HOME into envs if not exists
    let home_in_envs = envs.iter().any(|x| x.starts_with("HOME="));
    if !home_in_envs {
        if let Some(dir_home) = utils::get_user_home(proc.user().uid()) {
            envs.push(format!("HOME={}", dir_home.to_string_lossy()));
        }
    }

    // Reset the process env based on oci spec.
    env::vars().for_each(|(key, _value)| env::remove_var(key));
    utils::parse_env(&envs)
        .iter()
        .for_each(|(key, value)| env::set_var(key, value));

    // Initialize seccomp profile right before we are ready to execute the
    // payload so as few syscalls will happen between here and payload exec. The
    // notify socket will still need network related syscalls.
    #[cfg(feature = "libseccomp")]
    if let Some(seccomp) = linux.seccomp() {
        if proc.no_new_privileges().is_some() {
            let notify_fd =
                seccomp::initialize_seccomp(seccomp).context("failed to execute seccomp")?;
            sync_seccomp(notify_fd, main_sender, init_receiver)
                .context("failed to sync seccomp")?;
        }
    }
    #[cfg(not(feature = "libseccomp"))]
    if proc.no_new_privileges().is_some() {
        warn!("seccomp not available, unable to set seccomp privileges!")
    }

    // this checks if the binary to run actually exists and if we have permissions to run it.
    // Taken from https://github.com/opencontainers/runc/blob/25c9e888686773e7e06429133578038a9abc091d/libcontainer/standard_init_linux.go#L195-L206
    if let Some(args) = proc.args() {
        let path_var = {
            let mut ret: &str = "";
            for var in &envs {
                if var.starts_with("PATH=") {
                    ret = var;
                }
            }
            ret
        };
        let executable_path = utils::get_executable_path(&args[0], path_var);
        match executable_path {
            None => bail!(
                "executable '{}' for container process does not exist",
                args[0]
            ),
            Some(path) => {
                if !utils::is_executable(&path)? {
                    bail!("file {:?} does not have executable permission set", path);
                }
            }
        }
    }

    // Notify main process that the init process is ready to execute the
    // payload.  Note, because we are already inside the pid namespace, the pid
    // outside the pid namespace should be recorded by the intermediate process
    // already.
    main_sender.init_ready()?;
    main_sender
        .close()
        .context("failed to close down main sender in init process")?;

    // listing on the notify socket for container start command
    args.notify_socket.wait_for_container_start()?;
    args.notify_socket.close()?;

    // create_container hook needs to be called after the namespace setup, but
    // before pivot_root is called. This runs in the container namespaces.
    if matches!(args.container_type, ContainerType::InitContainer) {
        if let Some(hooks) = hooks {
            hooks::run_hooks(hooks.start_container().as_ref(), container)?
        }
    }

    if proc.args().is_some() {
        args.executor_manager.exec(spec)?;
        unreachable!("should not be back here");
    } else {
        bail!("on non-Windows, at least one process arg entry is required")
    }
}

// Before 3.19 it was possible for an unprivileged user to enter an user namespace,
// become root and then call setgroups in order to drop membership in supplementary
// groups. This allowed access to files which blocked access based on being a member
// of these groups (see CVE-2014-8989)
//
// This leaves us with three scenarios:
//
// Unprivileged user starting a rootless container: The main process is running as an
// unprivileged user and therefore cannot write the mapping until "deny" has been written
// to /proc/{pid}/setgroups. Once written /proc/{pid}/setgroups cannot be reset and the
// setgroups system call will be disabled for all processes in this user namespace. This
// also means that we should detect if the user is unprivileged and additional gids have
// been specified and bail out early as this can never work. This is not handled here,
// but during the validation for rootless containers.
//
// Privileged user starting a rootless container: It is not necessary to write "deny" to
// /proc/setgroups in order to create the gid mapping and therefore we don't. This means
// that setgroups could be used to drop groups, but this is fine as the user is privileged
// and could do so anyway.
// We already have checked during validation if the specified supplemental groups fall into
// the range that are specified in the gid mapping and bail out early if they do not.
//
// Privileged user starting a normal container: Just add the supplementary groups.
//
fn set_supplementary_gids(
    user: &User,
    rootless: &Option<Rootless>,
    syscall: &dyn Syscall,
) -> Result<()> {
    if let Some(additional_gids) = user.additional_gids() {
        if additional_gids.is_empty() {
            return Ok(());
        }

        let setgroups =
            fs::read_to_string("/proc/self/setgroups").context("failed to read setgroups")?;
        if setgroups.trim() == "deny" {
            bail!("cannot set supplementary gids, setgroup is disabled");
        }

        let gids: Vec<Gid> = additional_gids
            .iter()
            .map(|gid| Gid::from_raw(*gid))
            .collect();

        match rootless {
            Some(r) if r.privileged => {
                syscall.set_groups(&gids).with_context(|| {
                    format!("failed to set privileged supplementary gids: {gids:?}")
                })?;
            }
            None => {
                syscall.set_groups(&gids).with_context(|| {
                    format!("failed to set unprivileged supplementary gids: {gids:?}")
                })?;
            }
            // this should have been detected during validation
            _ => unreachable!(
                "unprivileged users cannot set supplementary gids in rootless container"
            ),
        }
    }

    Ok(())
}

#[cfg(feature = "libseccomp")]
fn sync_seccomp(
    fd: Option<i32>,
    main_sender: &mut channel::MainSender,
    init_receiver: &mut channel::InitReceiver,
) -> Result<()> {
    if let Some(fd) = fd {
        log::debug!("init process sync seccomp, notify fd: {}", fd);
        main_sender.seccomp_notify_request(fd)?;
        init_receiver.wait_for_seccomp_request_done()?;
        // Once we are sure the seccomp notify fd is sent, we can safely close
        // it. The fd is now duplicated to the main process and sent to seccomp
        // listener.
        let _ = unistd::close(fd);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::{
        syscall::create_syscall,
        test::{ArgName, MountArgs, TestHelperSyscall},
    };
    #[cfg(feature = "libseccomp")]
    use nix::unistd;
    use oci_spec::runtime::{LinuxNamespaceBuilder, SpecBuilder, UserBuilder};
    #[cfg(feature = "libseccomp")]
    use serial_test::serial;
    use std::fs;

    #[test]
    fn test_readonly_path() -> Result<()> {
        let syscall = create_syscall();
        readonly_path(Path::new("/proc/sys"), syscall.as_ref())?;

        let want = vec![
            MountArgs {
                source: Some(PathBuf::from("/proc/sys")),
                target: PathBuf::from("/proc/sys"),
                fstype: None,
                flags: MsFlags::MS_BIND | MsFlags::MS_REC,
                data: None,
            },
            MountArgs {
                source: Some(PathBuf::from("/proc/sys")),
                target: PathBuf::from("/proc/sys"),
                fstype: None,
                flags: MsFlags::MS_NOSUID
                    | MsFlags::MS_NODEV
                    | MsFlags::MS_NOEXEC
                    | MsFlags::MS_BIND
                    | MsFlags::MS_REMOUNT
                    | MsFlags::MS_RDONLY,
                data: None,
            },
        ];
        let got = syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_mount_args();

        assert_eq!(want, *got);
        assert_eq!(got.len(), 2);
        Ok(())
    }

    #[test]
    fn test_apply_rest_namespaces() -> Result<()> {
        let syscall = create_syscall();
        let spec = SpecBuilder::default().build()?;
        let linux_spaces = vec![
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Uts)
                .build()?,
            LinuxNamespaceBuilder::default()
                .typ(LinuxNamespaceType::Pid)
                .build()?,
        ];
        let namespaces = Namespaces::from(Some(&linux_spaces));

        apply_rest_namespaces(&namespaces, &spec, syscall.as_ref())?;

        let got_hostnames = syscall
            .as_ref()
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_hostname_args();
        assert_eq!(1, got_hostnames.len());
        assert_eq!("youki".to_string(), got_hostnames[0]);

        let got_domainnames = syscall
            .as_ref()
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap()
            .get_domainname_args();
        assert_eq!(0, got_domainnames.len());
        Ok(())
    }

    #[test]
    fn test_set_supplementary_gids() -> Result<()> {
        // gids additional gids is empty case
        let user = UserBuilder::default().build().unwrap();
        assert!(set_supplementary_gids(&user, &None, create_syscall().as_ref()).is_ok());

        let tests = vec![
            (
                UserBuilder::default()
                    .additional_gids(vec![33, 34])
                    .build()?,
                None::<Rootless>,
                vec![vec![Gid::from_raw(33), Gid::from_raw(34)]],
            ),
            // unreachable case
            (
                UserBuilder::default().build()?,
                Some(Rootless::default()),
                vec![],
            ),
            (
                UserBuilder::default()
                    .additional_gids(vec![37, 38])
                    .build()?,
                Some(Rootless {
                    privileged: true,
                    gid_mappings: None,
                    newgidmap: None,
                    newuidmap: None,
                    uid_mappings: None,
                    user_namespace: None,
                }),
                vec![vec![Gid::from_raw(37), Gid::from_raw(38)]],
            ),
        ];
        for (user, rootless, want) in tests.into_iter() {
            let syscall = create_syscall();
            let result = set_supplementary_gids(&user, &rootless, syscall.as_ref());
            match fs::read_to_string("/proc/self/setgroups")?.trim() {
                "deny" => {
                    assert!(result.is_err());
                }
                "allow" => {
                    assert!(result.is_ok());
                    let got = syscall
                        .as_any()
                        .downcast_ref::<TestHelperSyscall>()
                        .unwrap()
                        .get_groups_args();
                    assert_eq!(want, got);
                }
                _ => unreachable!("setgroups value unknown"),
            }
        }
        Ok(())
    }

    #[test]
    #[serial]
    #[cfg(feature = "libseccomp")]
    fn test_sync_seccomp() -> Result<()> {
        use std::os::unix::io::IntoRawFd;
        use std::thread;
        use utils::create_temp_dir;

        let tmp_dir = create_temp_dir("test_sync_seccomp")?;
        let tmp_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(tmp_dir.path().join("temp_file"))
            .expect("create temp file failed");

        let (mut main_sender, mut main_receiver) = channel::main_channel()?;
        let (mut init_sender, mut init_receiver) = channel::init_channel()?;

        let fd = tmp_file.into_raw_fd();
        let th = thread::spawn(move || {
            assert!(main_receiver.wait_for_seccomp_request().is_ok());
            assert!(init_sender.seccomp_notify_done().is_ok());
        });

        // sync_seccomp close the fd,
        sync_seccomp(Some(fd), &mut main_sender, &mut init_receiver)?;
        // so expecting close the same fd again will causing EBADF error.
        assert_eq!(nix::errno::Errno::EBADF, unistd::close(fd).unwrap_err());
        assert!(th.join().is_ok());
        Ok(())
    }

    #[test]
    fn test_masked_path_does_not_exist() {
        let syscall = create_syscall();
        let mocks = syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap();
        mocks.set_ret_err(ArgName::Mount, || {
            Err(SyscallError::MountFailed {
                mount_source: None,
                mount_target: PathBuf::new(),
                fstype: None,
                flags: MsFlags::empty(),
                data: None,
                errno: nix::errno::Errno::ENOENT,
            })
        });

        assert!(masked_path(Path::new("/proc/self"), &None, syscall.as_ref()).is_ok());
        let got = mocks.get_mount_args();
        assert_eq!(0, got.len());
    }

    #[test]
    fn test_masked_path_is_file_with_no_label() {
        let syscall = create_syscall();
        let mocks = syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap();
        mocks.set_ret_err(ArgName::Mount, || {
            Err(SyscallError::MountFailed {
                mount_source: None,
                mount_target: PathBuf::new(),
                fstype: None,
                flags: MsFlags::empty(),
                data: None,
                errno: nix::errno::Errno::ENOTDIR,
            })
        });

        assert!(masked_path(Path::new("/proc/self"), &None, syscall.as_ref()).is_ok());

        let got = mocks.get_mount_args();
        let want = MountArgs {
            source: Some(PathBuf::from("tmpfs")),
            target: PathBuf::from("/proc/self"),
            fstype: Some("tmpfs".to_string()),
            flags: MsFlags::MS_RDONLY,
            data: Some("".to_string()),
        };
        assert_eq!(1, got.len());
        assert_eq!(want, got[0]);
    }

    #[test]
    fn test_masked_path_is_file_with_label() {
        let syscall = create_syscall();
        let mocks = syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap();
        mocks.set_ret_err(ArgName::Mount, || {
            Err(SyscallError::MountFailed {
                mount_source: None,
                mount_target: PathBuf::new(),
                fstype: None,
                flags: MsFlags::empty(),
                data: None,
                errno: nix::errno::Errno::ENOTDIR,
            })
        });

        assert!(masked_path(
            Path::new("/proc/self"),
            &Some("default".to_string()),
            syscall.as_ref()
        )
        .is_ok());

        let got = mocks.get_mount_args();
        let want = MountArgs {
            source: Some(PathBuf::from("tmpfs")),
            target: PathBuf::from("/proc/self"),
            fstype: Some("tmpfs".to_string()),
            flags: MsFlags::MS_RDONLY,
            data: Some("context=\"default\"".to_string()),
        };
        assert_eq!(1, got.len());
        assert_eq!(want, got[0]);
    }

    #[test]
    fn test_masked_path_with_unknown_error() {
        let syscall = create_syscall();
        let mocks = syscall
            .as_any()
            .downcast_ref::<TestHelperSyscall>()
            .unwrap();
        mocks.set_ret_err(ArgName::Mount, || {
            Err(SyscallError::MountFailed {
                mount_source: None,
                mount_target: PathBuf::new(),
                fstype: None,
                flags: MsFlags::empty(),
                data: None,
                errno: nix::errno::Errno::UnknownErrno,
            })
        });

        assert!(masked_path(Path::new("/proc/self"), &None, syscall.as_ref()).is_err());
        let got = mocks.get_mount_args();
        assert_eq!(0, got.len());
    }
}
