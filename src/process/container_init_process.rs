use super::args::ContainerArgs;
use crate::apparmor;
use crate::syscall::Syscall;
use crate::{
    capabilities, hooks, namespaces::Namespaces, process::channel, rootfs::RootFS,
    rootless::Rootless, seccomp, tty, utils,
};
use anyhow::{bail, Context, Result};
use nix::mount::mount as nix_mount;
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
fn readonly_path(path: &str) -> Result<()> {
    match nix_mount::<str, str, str, str>(
        Some(path),
        path,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    ) {
        // ignore error if path is not exist.
        Err(nix::errno::Errno::ENOENT) => {
            log::warn!("readonly path {:?} not exist", path);
            return Ok(());
        }
        Err(err) => bail!(err),
        Ok(_) => {}
    }

    nix_mount::<str, str, str, str>(
        Some(path),
        path,
        None::<&str>,
        MsFlags::MS_NOSUID
            | MsFlags::MS_NODEV
            | MsFlags::MS_NOEXEC
            | MsFlags::MS_BIND
            | MsFlags::MS_REMOUNT
            | MsFlags::MS_RDONLY,
        None::<&str>,
    )?;
    log::debug!("readonly path {:?} mounted", path);
    Ok(())
}

// For files, bind mounts /dev/null over the top of the specified path.
// For directories, mounts read-only tmpfs over the top of the specified path.
fn masked_path(path: &str, mount_label: &Option<String>) -> Result<()> {
    match nix_mount::<str, str, str, str>(
        Some("/dev/null"),
        path,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    ) {
        // ignore error if path is not exist.
        Err(nix::errno::Errno::ENOENT) => {
            log::warn!("masked path {:?} not exist", path);
            return Ok(());
        }
        Err(nix::errno::Errno::ENOTDIR) => {
            let label = match mount_label {
                Some(l) => format!("context={}", l),
                None => "".to_string(),
            };
            let _ = nix_mount(
                Some("tmpfs"),
                path,
                Some("tmpfs"),
                MsFlags::MS_RDONLY,
                Some(label.as_str()),
            );
        }
        Err(err) => bail!(err),
        Ok(_) => {}
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
    intermediate_sender: &mut channel::IntermediateSender,
    _init_receiver: &mut channel::InitReceiver,
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
        nix_mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT | MsFlags::MS_BIND,
            None::<&str>,
        )?
    }

    if let Some(paths) = linux.readonly_paths() {
        // mount readonly path
        for path in paths {
            readonly_path(path).context("Failed to set read only path")?;
        }
    }

    if let Some(paths) = linux.masked_paths() {
        // mount masked path
        for path in paths {
            masked_path(path, linux.mount_label()).context("Failed to set masked path")?;
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
            Err(e) => bail!("Failed to chdir: {}", e),
        }
    };

    set_supplementary_gids(proc.user(), &args.rootless)
        .context("failed to set supplementary gids")?;

    syscall
        .set_id(
            Uid::from_raw(proc.user().uid()),
            Gid::from_raw(proc.user().gid()),
        )
        .context("Failed to configure uid and gid")?;

    // Without no new privileges, seccomp is a privileged operation. We have to
    // do this before dropping capabilities. Otherwise, we should do it later,
    // as close to exec as possible.
    if linux.seccomp().is_some() && proc.no_new_privileges().is_none() {
        seccomp::initialize_seccomp(linux.seccomp().as_ref().unwrap())
            .context("Failed to execute seccomp")?;
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

    // clean up and handle perserved fds.
    cleanup_file_descriptors(preserve_fds).with_context(|| "Failed to clean up extra fds")?;

    // change directory to process.cwd if process.cwd is not empty
    if do_chdir {
        unistd::chdir(proc.cwd()).with_context(|| format!("failed to chdir {:?}", proc.cwd()))?;
    }

    // Reset the process env based on oci spec.
    env::vars().for_each(|(key, _value)| std::env::remove_var(key));
    utils::parse_env(&envs)
        .iter()
        .for_each(|(key, value)| env::set_var(key, value));

    // notify parents that the init process is ready to execute the payload.
    // Note, we pass -1 here because we are already inside the pid namespace.
    // The pid outside the pid namespace should be recorded by the intermediate
    // process.
    intermediate_sender.init_ready()?;

    // listing on the notify socket for container start command
    args.notify_socket.wait_for_container_start()?;

    // create_container hook needs to be called after the namespace setup, but
    // before pivot_root is called. This runs in the container namespaces.
    if args.init {
        if let Some(hooks) = hooks {
            hooks::run_hooks(hooks.start_container().as_ref(), container)?
        }
    }

    if let Some(seccomp) = linux.seccomp() {
        if proc.no_new_privileges().is_some() {
            // Initialize seccomp profile right before we are ready to execute the
            // payload. The notify socket will still need network related syscalls.
            seccomp::initialize_seccomp(seccomp).context("Failed to execute seccomp")?;
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
fn set_supplementary_gids(user: &User, rootless: &Option<Rootless>) -> Result<()> {
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
                nix::unistd::setgroups(&gids).context("failed to set supplementary gids")?;
            }
            None => {
                nix::unistd::setgroups(&gids).context("failed to set supplementary gids")?;
            }
            // this should have been detected during validation
            _ => unreachable!(
                "unprivileged users cannot set supplementary gids in rootless container"
            ),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, Result};
    use nix::{fcntl, sys, unistd};
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
}
