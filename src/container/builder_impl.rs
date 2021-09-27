use crate::{
    hooks,
    notify_socket::NotifyListener,
    process::{args::ContainerArgs, channel, fork, intermediate},
    rootless::Rootless,
    syscall::Syscall,
    utils,
};
use anyhow::{bail, Context, Result};
use cgroups::{self, common::CgroupManager};
use nix::unistd::Pid;
use oci_spec::runtime::{LinuxResources, Spec};
use std::{fs, io::Write, os::unix::prelude::RawFd, path::PathBuf};

use super::{Container, ContainerStatus};

pub(super) struct ContainerBuilderImpl<'a> {
    /// Flag indicating if an init or a tenant container should be created
    pub init: bool,
    /// Interface to operating system primitives
    pub syscall: &'a dyn Syscall,
    /// Flag indicating if systemd should be used for cgroup management
    pub use_systemd: bool,
    /// Id of the container
    pub container_id: String,
    /// OCI complient runtime spec
    pub spec: &'a Spec,
    /// Root filesystem of the container
    pub rootfs: PathBuf,
    /// File which will be used to communicate the pid of the
    /// container process to the higher level runtime
    pub pid_file: Option<PathBuf>,
    /// Socket to communicate the file descriptor of the ptty
    pub console_socket: Option<RawFd>,
    /// Options for rootless containers
    pub rootless: Option<Rootless<'a>>,
    /// Path to the Unix Domain Socket to communicate container start
    pub notify_path: PathBuf,
    /// Container state
    pub container: Option<Container>,
    /// File descriptos preserved/passed to the container init process.
    pub preserve_fds: i32,
}

impl<'a> ContainerBuilderImpl<'a> {
    pub(super) fn create(&mut self) -> Result<()> {
        if let Err(outer) = self.run_container().context("failed to create container") {
            if let Err(inner) = self.cleanup_container() {
                return Err(outer.context(inner));
            }

            return Err(outer);
        }

        Ok(())
    }

    fn run_container(&mut self) -> Result<()> {
        let linux = self.spec.linux().as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(linux.cgroups_path(), &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;
        let process = self.spec.process().as_ref().context("No process in spec")?;

        if self.init {
            if let Some(hooks) = self.spec.hooks().as_ref() {
                hooks::run_hooks(hooks.create_runtime().as_ref(), self.container.as_ref())?
            }
        }

        // We use a set of channels to communicate between parent and child process. Each channel is uni-directional.
        let (sender_to_intermediate, receiver_from_main) = &mut channel::main_to_intermediate()?;
        let (sender_to_main, receiver_from_intermediate) = &mut channel::intermediate_to_main()?;

        // Need to create the notify socket before we pivot root, since the unix
        // domain socket used here is outside of the rootfs of container. During
        // exec, need to create the socket before we enter into existing mount
        // namespace.
        let notify_socket: NotifyListener = NotifyListener::new(&self.notify_path)?;

        // If Out-of-memory score adjustment is set in specification.  set the score
        // value for the current process check
        // https://dev.to/rrampage/surviving-the-linux-oom-killer-2ki9 for some more
        // information.
        //
        // This has to be done before !dumpable because /proc/self/oom_score_adj
        // is not writeable unless you're an privileged user (if !dumpable is
        // set). All children inherit their parent's oom_score_adj value on
        // fork(2) so this will always be propagated properly.
        if let Some(oom_score_adj) = process.oom_score_adj() {
            log::debug!("Set OOM score to {}", oom_score_adj);
            let mut f = fs::File::create("/proc/self/oom_score_adj")?;
            f.write_all(oom_score_adj.to_string().as_bytes())?;
        }

        // Make the process non-dumpable, to avoid various race conditions that
        // could cause processes in namespaces we're joining to access host
        // resources (or potentially execute code).
        //
        // However, if the number of namespaces we are joining is 0, we are not
        // going to be switching to a different security context. Thus setting
        // ourselves to be non-dumpable only breaks things (like rootless
        // containers), which is the recommendation from the kernel folks.
        if linux.namespaces().is_some() {
            prctl::set_dumpable(false).unwrap();
        }

        // This intermediate_args will be passed to the container intermediate process,
        // therefore we will have to move all the variable by value. Since self
        // is a shared reference, we have to clone these variables here.
        let intermediate_args = ContainerArgs {
            init: self.init,
            syscall: self.syscall,
            spec: self.spec.clone(),
            rootfs: self.rootfs.clone(),
            console_socket: self.console_socket,
            notify_socket,
            preserve_fds: self.preserve_fds,
            container: self.container.clone(),
            rootless: self.rootless.clone(),
        };
        let intermediate_pid = fork::container_fork(|| {
            // The fds in the pipe is duplicated during fork, so we first close
            // the unused fds. Note, this already runs in the child process.
            sender_to_intermediate
                .close()
                .context("Failed to close unused sender")?;
            receiver_from_intermediate
                .close()
                .context("Failed to close unused receiver")?;

            intermediate::container_intermediate(
                intermediate_args,
                receiver_from_main,
                sender_to_main,
            )
        })?;
        // Close down unused fds. The corresponding fds are duplicated to the
        // child process during fork.
        receiver_from_main
            .close()
            .context("Failed to close parent to child receiver")?;
        sender_to_main
            .close()
            .context("Failed to close child to parent sender")?;

        // If creating a rootless container, the intermediate process will ask
        // the main process to set up uid and gid mapping, once the intermediate
        // process enters into a new user namespace.
        if let Some(rootless) = self.rootless.as_ref() {
            receiver_from_intermediate.wait_for_mapping_request()?;
            setup_mapping(rootless, intermediate_pid)?;
            sender_to_intermediate.mapping_written()?;
        }

        let init_pid = receiver_from_intermediate.wait_for_intermediate_ready()?;
        log::debug!("init pid is {:?}", init_pid);

        if self.rootless.is_none() && linux.resources().is_some() && self.init {
            if let Some(resources) = linux.resources().as_ref() {
                apply_cgroups(resources, init_pid, cmanager.as_ref())?;
            }
        }

        // if file to write the pid to is specified, write pid of the child
        if let Some(pid_file) = &self.pid_file {
            fs::write(&pid_file, format!("{}", init_pid)).context("Failed to write pid file")?;
        }

        if let Some(container) = &mut self.container {
            // update status and pid of the container process
            container
                .set_status(ContainerStatus::Created)
                .set_creator(nix::unistd::geteuid().as_raw())
                .set_pid(init_pid.as_raw())
                .save()
                .context("Failed to save container state")?;
        }

        Ok(())
    }

    fn cleanup_container(&self) -> Result<()> {
        let linux = self.spec.linux.as_ref().context("no linux in spec")?;
        let cgroups_path = utils::get_cgroup_path(&linux.cgroups_path, &self.container_id);
        let cmanager = cgroups::common::create_cgroup_manager(&cgroups_path, self.use_systemd)?;

        let mut errors = Vec::new();
        if let Err(e) = cmanager.remove().context("failed to remove cgroup") {
            errors.push(e.to_string());
        }

        if let Some(container) = &self.container {
            if container.root.exists() {
                if let Err(e) = fs::remove_dir_all(&container.root)
                    .with_context(|| format!("could not delete {}", container.root.display()))
                {
                    errors.push(e.to_string());
                }
            }
        }

        if !errors.is_empty() {
            bail!("failed to cleanup container: {}", errors.join(";"));
        }

        Ok(())
    }
}

fn setup_mapping(rootless: &Rootless, pid: Pid) -> Result<()> {
    log::debug!("write mapping for pid {:?}", pid);
    if !rootless.privileged {
        // The main process is running as an unprivileged user and cannot write the mapping
        // until "deny" has been written to setgroups. See CVE-2014-8989.
        utils::write_file(format!("/proc/{}/setgroups", pid), "deny")?;
    }
    rootless
        .write_uid_mapping(pid)
        .context(format!("failed to map uid of pid {}", pid))?;
    rootless
        .write_gid_mapping(pid)
        .context(format!("failed to map gid of pid {}", pid))?;
    Ok(())
}

fn apply_cgroups<C: CgroupManager + ?Sized>(
    resources: &LinuxResources,
    pid: Pid,
    cmanager: &C,
) -> Result<()> {
    let controller_opt = cgroups::common::ControllerOpt {
        resources,
        freezer_state: None,
        oom_score_adj: None,
        disable_oom_killer: false,
    };
    cmanager
        .add_task(pid)
        .with_context(|| format!("failed to add task {} to cgroup manager", pid))?;

    cmanager
        .apply(&controller_opt)
        .context("failed to apply resource limits to cgroup")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use cgroups::test_manager::TestManager;
    use nix::{
        sched::{unshare, CloneFlags},
        unistd::{self, getgid, getuid},
    };
    use oci_spec::runtime::LinuxIdMappingBuilder;
    use serial_test::serial;

    use crate::process::channel::{intermediate_to_main, main_to_intermediate};

    use super::*;

    #[test]
    #[serial]
    fn setup_uid_mapping_should_succeed() -> Result<()> {
        let uid_mapping = LinuxIdMappingBuilder::default()
            .host_id(getuid())
            .container_id(0u32)
            .size(1u32)
            .build()?;
        let uid_mappings = vec![uid_mapping];
        let rootless = Rootless {
            uid_mappings: Some(&uid_mappings),
            privileged: true,
            ..Default::default()
        };
        let (mut sender_to_parent, mut receiver_from_child) = intermediate_to_main()?;
        let (mut sender_to_child, mut receiver_from_parent) = main_to_intermediate()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                receiver_from_child.wait_for_mapping_request()?;
                receiver_from_child.close()?;
                setup_mapping(&rootless, child)?;
                let line = fs::read_to_string(format!("/proc/{}/uid_map", child.as_raw()))?;
                let line_splited = line.split_whitespace();
                for (act, expect) in line_splited.zip([
                    uid_mapping.container_id().to_string().as_str(),
                    uid_mapping.host_id().to_string().as_str(),
                    uid_mapping.size().to_string().as_str(),
                ]) {
                    assert_eq!(act, expect);
                }
                sender_to_child.mapping_written()?;
                sender_to_child.close()?;
            }
            unistd::ForkResult::Child => {
                prctl::set_dumpable(true).unwrap();
                unshare(CloneFlags::CLONE_NEWUSER)?;
                sender_to_parent.identifier_mapping_request()?;
                sender_to_parent.close()?;
                receiver_from_parent.wait_for_mapping_ack()?;
                receiver_from_child.close()?;
                std::process::exit(0);
            }
        }
        Ok(())
    }

    #[test]
    #[serial]
    fn setup_gid_mapping_should_successed() -> Result<()> {
        let gid_mapping = LinuxIdMappingBuilder::default()
            .host_id(getgid())
            .container_id(0u32)
            .size(1u32)
            .build()?;
        let gid_mappings = vec![gid_mapping];
        let rootless = Rootless {
            gid_mappings: Some(&gid_mappings),
            ..Default::default()
        };
        let (mut sender_to_parent, mut receiver_from_child) = intermediate_to_main()?;
        let (mut sender_to_child, mut receiver_from_parent) = main_to_intermediate()?;
        match unsafe { unistd::fork()? } {
            unistd::ForkResult::Parent { child } => {
                receiver_from_child.wait_for_mapping_request()?;
                receiver_from_child.close()?;
                setup_mapping(&rootless, child)?;
                let line = fs::read_to_string(format!("/proc/{}/gid_map", child.as_raw()))?;
                let line_splited = line.split_whitespace();
                for (act, expect) in line_splited.zip([
                    gid_mapping.container_id().to_string().as_str(),
                    gid_mapping.host_id().to_string().as_str(),
                    gid_mapping.size().to_string().as_str(),
                ]) {
                    assert_eq!(act, expect);
                }
                assert_eq!(
                    fs::read_to_string(format!("/proc/{}/setgroups", child.as_raw()))?,
                    "deny\n",
                );
                sender_to_child.mapping_written()?;
                sender_to_child.close()?;
            }
            unistd::ForkResult::Child => {
                prctl::set_dumpable(true).unwrap();
                unshare(CloneFlags::CLONE_NEWUSER)?;
                sender_to_parent.identifier_mapping_request()?;
                sender_to_parent.close()?;
                receiver_from_parent.wait_for_mapping_ack()?;
                receiver_from_child.close()?;
                std::process::exit(0);
            }
        }
        Ok(())
    }

    #[test]
    fn apply_cgroup_successed() -> Result<()> {
        let cmanager = TestManager::default();
        let sample_pid = Pid::from_raw(1000);
        let resources = LinuxResources::default();
        apply_cgroups(&resources, sample_pid, &cmanager)?;
        assert_eq!(cmanager.get_add_task_args(), vec![sample_pid]);
        Ok(())
    }
}
