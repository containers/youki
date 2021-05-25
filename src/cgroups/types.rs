use nix::unistd::Pid;

use oci_spec::LinuxResources;

pub trait CgroupManager {
    fn apply(&self, linux_resources: &LinuxResources, pid: Pid);
}