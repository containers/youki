use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::process::exit;

use anyhow::Result;

use anyhow::bail;
use child::ChildProcess;
use init::InitProcess;
use nix::sched;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd;

use crate::cgroups::common::CgroupManager;
use crate::container::ContainerStatus;
use crate::process::{child, init, parent, Process};
use crate::utils;
use crate::{cond::Cond, container::Container};

pub fn fork_first<P: AsRef<Path>>(
    pid_file: Option<P>,
    is_userns: bool,
    linux: &oci_spec::Linux,
    container: &Container,
    cmanager: Box<dyn CgroupManager>,
) -> Result<Process> {
    let ccond = Cond::new()?;

    let (mut parent, sender_for_parent) = parent::ParentProcess::new()?;
    let child = child::ChildProcess::new(sender_for_parent)?;

    unsafe {
        match unistd::fork()? {
            unistd::ForkResult::Child => {
                utils::set_name("rc-user")?;

                if let Some(ref r) = linux.resources {
                    if let Some(adj) = r.oom_score_adj {
                        let mut f = fs::File::create("/proc/self/oom_score_adj")?;
                        f.write_all(adj.to_string().as_bytes())?;
                    }
                }

                if is_userns {
                    sched::unshare(sched::CloneFlags::CLONE_NEWUSER)?;
                }

                ccond.notify()?;

                Ok(Process::Child(child))
            }
            unistd::ForkResult::Parent { child } => {
                ccond.wait()?;

                cmanager.apply(&linux.resources.as_ref().unwrap(), child)?;

                let init_pid = parent.wait_for_child_ready()?;
                container
                    .update_status(ContainerStatus::Created)?
                    .set_pid(init_pid)
                    .save()?;

                if let Some(pid_file) = pid_file {
                    fs::write(&pid_file, format!("{}", child))?;
                }
                Ok(Process::Parent(parent))
            }
        }
    }
}

pub fn fork_init(mut child_process: ChildProcess) -> Result<Process> {
    let sender_for_child = child_process.setup_uds()?;
    unsafe {
        match unistd::fork()? {
            unistd::ForkResult::Child => Ok(Process::Init(InitProcess::new(sender_for_child))),
            unistd::ForkResult::Parent { child } => {
                child_process.wait_for_init_ready()?;
                child_process.ready(child)?;

                match waitpid(child, None)? {
                    WaitStatus::Exited(pid, status) => {
                        // cmanager.remove()?;
                        log::debug!("exited pid: {:?}, status: {:?}", pid, status);
                        exit(status);
                    }
                    WaitStatus::Signaled(pid, status, _) => {
                        log::debug!("signaled pid: {:?}, status: {:?}", pid, status);
                        exit(0);
                    }
                    _ => bail!("abnormal exited!"),
                }
            }
        }
    }
}
