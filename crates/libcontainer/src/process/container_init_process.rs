use super::args::ContainerArgs;
use crate::apparmor;
use crate::syscall::Syscall;
use crate::{
    capabilities, hooks, namespaces::Namespaces, process::channel, rootfs::RootFS,
    rootless::Rootless, seccomp, tty, utils,
};
use anyhow::{bail, Context, Result};
use nix::mount::MsFlags;
use nix::sched::CloneFlags;
use nix::{
    fcntl,
    unistd::{self, Gid, Uid},
};
use oci_spec::runtime::{LinuxNamespaceType, Spec, User};
use std::collections::HashMap;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

// Get a list of open fds for the calling process.
fn get_open_fds() -> Result<Vec<i32>> {
    const PROCFS_FD_PATH: &str = "/proc/self/fd";
    utils::ensure_procfs(Path::new(PROCFS_FD_PATH))
        .with_context(|| format!("{} is not the actual procfs", PROCFS_FD_PATH))?;

    let fds: Vec<i32> = fs::read_dir(PROCFS_FD_PATH)?
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry.path()),
            Err(_) => None,
        })
        .filter_map(|path| path.file_name().map(|file_name| file_name.to_owned()))
        .filter_map(|file_name| file_name.to_str().map(String::from))
        .filter_map(|file_name| -> Option<i32> {
            // Convert the file name from string into i32. Since we are looking
            // at /proc/<pid>/fd, anything that's not a number (i32) can be
            // ignored. We are only interested in opened fds.
            match file_name.parse() {
                Ok(fd) => Some(fd),
                Err(_) => None,
            }
        })
        .collect();

    Ok(fds)
}

// Cleanup any extra file descriptors, so the new container process will not
// leak a file descriptor from before execve gets executed. The first 3 fd will
// stay open: stdio, stdout, and stderr. We would further preserve the next
// "preserve_fds" number of fds. Set the rest of fd with CLOEXEC flag, so they
// will be closed after execve into the container payload. We can't close the
// fds immediatly since we at least still need it for the pipe used to wait on
// starting the container.
fn cleanup_file_descriptors(preserve_fds: i32) -> Result<()> {
    let open_fds = get_open_fds().with_context(|| "Failed to obtain opened fds")?;
    // Include stdin, stdout, and stderr for fd 0, 1, and 2 respectively.
    let min_fd = preserve_fds + 3;
    let to_be_cleaned_up_fds: Vec<i32> = open_fds
        .iter()
        .filter_map(|&fd| if fd >= min_fd { Some(fd) } else { None })
        .collect();

    to_be_cleaned_up_fds.iter().for_each(|&fd| {
        // Intentionally ignore errors here -- the cases where this might fail
        // are basically file descriptors that have already been closed.
        let _ = fcntl::fcntl(fd, fcntl::F_SETFD(fcntl::FdFlag::FD_CLOEXEC));
    });

    Ok(())
}

fn sysctl(kernel_params: &HashMap<String, String>) -> Result<()> {
    let sys = PathBuf::from("/proc/sys");
    for (kernel_param, value) in kernel_params {
        let path = sys.join(kernel_param.replace(".", "/"));
        log::debug!(
            "apply value {} to kernel parameter {}.",
            value,
            kernel_param
        );
        fs::write(path, value.as_bytes())
            .with_context(|| format!("failed to set sysctl {}={}", kernel_param, value))?;
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
        if let Some(errno) = err.downcast_ref() {
            // ignore error if path is not exist.
            if matches!(errno, nix::errno::Errno::ENOENT) {
                return Ok(());
            }
        }
        bail!(err)
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
fn masked_path(path: &str, mount_label: &Option<String>, syscall: &dyn Syscall) -> Result<()> {
    if let Err(e) = syscall.mount(
        Some(Path::new("/dev/null")),
        Path::new(path),
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    ) {
        if let Some(errno) = e.downcast_ref() {
            if matches!(errno, nix::errno::Errno::ENOENT) {
                log::warn!("masked path {:?} not exist", path);
            } else if matches!(errno, nix::errno::Errno::ENOTDIR) {
                let label = match mount_label {
                    Some(l) => format!("context=\"{}\"", l),
                    None => "".to_string(),
                };
                syscall.mount(
                    Some(Path::new("tmpfs")),
                    Path::new(path),
                    Some("tmpfs"),
                    MsFlags::MS_RDONLY,
                    Some(label.as_str()),
                )?;
            }
        } else {
            bail!(e)
        }
    };
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
        }
    }
    Ok(())
}

pub fn container_init_process(
    args: &ContainerArgs,
    main_sender: &mut channel::MainSender,
    init_receiver: &mut channel::InitReceiver,
) -> Result<()> {
    let syscall = args.syscall;
    let spec = &args.spec;
    let linux = spec.linux().as_ref().context("no linux in spec")?;
    let proc = spec.process().as_ref().context("no process in spec")?;
    let mut envs: Vec<String> = proc.env().as_ref().unwrap_or(&vec![]).clone();
    let rootfs_path = &args.rootfs;
    let hooks = spec.hooks().as_ref();
    let container = args.container.as_ref();
    let namespaces = Namespaces::from(linux.namespaces().as_ref());

    // set up tty if specified
    if let Some(csocketfd) = args.console_socket {
        tty::setup_console(&csocketfd).with_context(|| "Failed to set up tty")?;
    }

    apply_rest_namespaces(&namespaces, spec, syscall)?;

    if let Some(true) = proc.no_new_privileges() {
        let _ = prctl::set_no_new_privileges(true);
    }

    if args.init {
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
                .with_context(|| format!("Failed to pivot root to {:?}", rootfs_path))?;
        } else {
            syscall
                .chroot(rootfs_path)
                .with_context(|| format!("Failed to chroot to {:?}", rootfs_path))?;
        }

        rootfs
            .adjust_root_mount_propagation(linux)
            .context("Failed to set propagation type of root mount")?;

        if let Some(kernel_params) = linux.sysctl() {
            sysctl(kernel_params)
                .with_context(|| format!("Failed to sysctl: {:?}", kernel_params))?;
        }
    }

    if let Some(profile) = proc.apparmor_profile() {
        apparmor::apply_profile(profile)
            .with_context(|| format!("failed to apply apparmor profile {}", profile))?;
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

    if let Some(paths) = linux.readonly_paths() {
        // mount readonly path
        for path in paths {
            readonly_path(Path::new(path), syscall)
                .with_context(|| format!("Failed to set read only path {:?}", path))?;
        }
    }

    if let Some(paths) = linux.masked_paths() {
        // mount masked path
        for path in paths {
            masked_path(path, linux.mount_label(), syscall).context("Failed to set masked path")?;
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
            Ok(_) => false,
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

    // Without no new privileges, seccomp is a privileged operation. We have to
    // do this before dropping capabilities. Otherwise, we should do it later,
    // as close to exec as possible.
    if let Some(seccomp) = linux.seccomp() {
        if proc.no_new_privileges().is_none() {
            let notify_fd =
                seccomp::initialize_seccomp(seccomp).context("failed to execute seccomp")?;
            sync_seccomp(notify_fd, main_sender, init_receiver)
                .context("failed to sync seccomp")?;
        }
    }

    capabilities::reset_effective(syscall).context("Failed to reset effective capabilities")?;
    if let Some(caps) = proc.capabilities() {
        capabilities::drop_privileges(caps, syscall).context("Failed to drop capabilities")?;
    }

    // Take care of LISTEN_FDS used for systemd-active-socket. If the value is
    // not 0, then we have to preserve those fds as well, and set up the correct
    // environment variables.
    let preserve_fds: i32 = match env::var("LISTEN_FDS") {
        Ok(listen_fds_str) => {
            let listen_fds = match listen_fds_str.parse::<i32>() {
                Ok(v) => v,
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
                    format!("LISTEN_FDS={}", listen_fds),
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

    // Clean up and handle perserved fds. We only mark the fd as CLOSEXEC, so we
    // don't have to worry about when the fd will be closed.
    cleanup_file_descriptors(preserve_fds).with_context(|| "Failed to clean up extra fds")?;

    // Change directory to process.cwd if process.cwd is not empty
    if do_chdir {
        unistd::chdir(proc.cwd()).with_context(|| format!("failed to chdir {:?}", proc.cwd()))?;
    }

    // Reset the process env based on oci spec.
    env::vars().for_each(|(key, _value)| std::env::remove_var(key));
    utils::parse_env(&envs)
        .iter()
        .for_each(|(key, value)| env::set_var(key, value));

    // Initialize seccomp profile right before we are ready to execute the
    // payload so as few syscalls will happen between here and payload exec. The
    // notify socket will still need network related syscalls.
    if let Some(seccomp) = linux.seccomp() {
        if proc.no_new_privileges().is_some() {
            let notify_fd =
                seccomp::initialize_seccomp(seccomp).context("failed to execute seccomp")?;
            sync_seccomp(notify_fd, main_sender, init_receiver)
                .context("failed to sync seccomp")?;
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

    // create_container hook needs to be called after the namespace setup, but
    // before pivot_root is called. This runs in the container namespaces.
    if args.init {
        if let Some(hooks) = hooks {
            hooks::run_hooks(hooks.start_container().as_ref(), container)?
        }
    }

    if let Some(args) = proc.args() {
        utils::do_exec(&args[0], args)?;
    } else {
        bail!("on non-Windows, at least one process arg entry is required")
    }

    // After do_exec is called, the process is replaced with the container
    // payload through execvp, so it should never reach here.
    unreachable!();
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
                    format!("failed to set privileged supplementary gids: {:?}", gids)
                })?;
            }
            None => {
                syscall.set_groups(&gids).with_context(|| {
                    format!("failed to set unprivileged supplementary gids: {:?}", gids)
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
        test::{MountArgs, TestHelperSyscall},
    };
    use anyhow::anyhow;
    use nix::{fcntl, sys, unistd};
    use oci_spec::runtime::{LinuxNamespaceBuilder, SpecBuilder, UserBuilder};
    use serial_test::serial;
    use std::{fs, os::unix::prelude::AsRawFd};

    // Note: We have to run these tests here as serial. The main issue is that
    // these tests has a dependency on the system state. The
    // cleanup_file_descriptors test is especially evil when running with other
    // tests because it would ran around close down different fds.

    #[test]
    #[serial]
    fn test_get_open_fds() -> Result<()> {
        let file = fs::File::open("/dev/null")?;
        let fd = file.as_raw_fd();
        let open_fds = super::get_open_fds()?;

        if !open_fds.iter().any(|&v| v == fd) {
            bail!("Failed to find the opened dev null fds: {:?}", open_fds);
        }

        // explicitly close the file before the test case returns.
        drop(file);

        // The stdio fds should also be contained in the list of opened fds.
        if !vec![0, 1, 2]
            .iter()
            .all(|&stdio_fd| open_fds.iter().any(|&open_fd| open_fd == stdio_fd))
        {
            bail!("Failed to find the stdio fds: {:?}", open_fds);
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn test_cleanup_file_descriptors() -> Result<()> {
        // Open a fd without the CLOEXEC flag. Rust automatically adds the flag,
        // so we use fcntl::open here for more control.
        let fd = fcntl::open("/dev/null", fcntl::OFlag::O_RDWR, sys::stat::Mode::empty())?;
        cleanup_file_descriptors(fd - 1).with_context(|| "Failed to clean up the fds")?;
        let fd_flag = fcntl::fcntl(fd, fcntl::F_GETFD)?;
        if (fd_flag & fcntl::FdFlag::FD_CLOEXEC.bits()) != 0 {
            bail!("CLOEXEC flag is not set correctly");
        }

        unistd::close(fd)?;
        Ok(())
    }

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
    fn test_masked_path() {
        // Errno::ENOENT case
        {
            let syscall = create_syscall();
            syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .set_mount_ret_err(Some(|| Err(anyhow!(nix::errno::Errno::ENOENT))), 1);
            assert!(masked_path("/proc/self", &None, syscall.as_ref()).is_ok());
            let got = syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args();
            assert_eq!(0, got.len());
        }
        // Errno::ENOTDIR with no label
        {
            let syscall = create_syscall();
            syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .set_mount_ret_err(Some(|| Err(anyhow!(nix::errno::Errno::ENOTDIR))), 1);
            assert!(masked_path("/proc/self", &None, syscall.as_ref()).is_ok());
            let got = syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args();
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
        // Errno::ENOTDIR with label
        {
            let syscall = create_syscall();
            syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .set_mount_ret_err(Some(|| Err(anyhow!(nix::errno::Errno::ENOTDIR))), 1);
            assert!(
                masked_path("/proc/self", &Some("default".to_string()), syscall.as_ref()).is_ok()
            );
            let got = syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args();
            let want = MountArgs {
                source: Some(PathBuf::from("tmpfs")),
                target: PathBuf::from("/proc/self"),
                fstype: Some("tmpfs".to_string()),
                flags: MsFlags::MS_RDONLY,
                data: Some("context=default".to_string()),
            };
            assert_eq!(1, got.len());
            assert_eq!(want, got[0]);
        }
        {
            let syscall = create_syscall();
            syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .set_mount_ret_err(Some(|| Err(anyhow!("unknown error"))), 1);
            assert!(masked_path("/proc/self", &None, syscall.as_ref()).is_err());
            let got = syscall
                .as_any()
                .downcast_ref::<TestHelperSyscall>()
                .unwrap()
                .get_mount_args();
            assert_eq!(0, got.len());
        }
    }
}
