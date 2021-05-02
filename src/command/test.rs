use super::Command;

pub struct TestHelperCommand;

impl Command for TestHelperCommand {
    fn pivot_rootfs(&self, _path: &std::path::Path) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn set_ns(&self, _rawfd: i32, _nstype: nix::sched::CloneFlags) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn set_id(&self, _uid: nix::unistd::Uid, _gid: nix::unistd::Gid) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn unshare(&self, _flags: nix::sched::CloneFlags) -> anyhow::Result<()> {
        unimplemented!()
    }
}
