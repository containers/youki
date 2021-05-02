use anyhow::Result;
use nix::{
    fcntl,
    sched::{self, CloneFlags},
    sys::stat,
    unistd::{self, Gid, Uid},
};

use crate::{
    command::{linux::LinuxCommand, test::TestHelperCommand, Command},
    spec::{LinuxNamespace, LinuxNamespaceType},
};

pub struct Namespaces {
    spaces: Vec<LinuxNamespace>,
    command: Box<dyn Command>,
    pub clone_flags: CloneFlags,
}

impl From<Vec<LinuxNamespace>> for Namespaces {
    fn from(namespaces: Vec<LinuxNamespace>) -> Self {
        let clone_flags = namespaces.iter().filter(|ns| ns.path.is_none()).fold(
            CloneFlags::empty(),
            |mut cf, ns| {
                cf |= CloneFlags::from_bits_truncate(ns.typ as i32);
                cf
            },
        );
        let command: Box<dyn Command> = if cfg!(test) {
            Box::new(TestHelperCommand)
        } else {
            Box::new(LinuxCommand)
        };

        Namespaces {
            spaces: namespaces,
            command,
            clone_flags,
        }
    }
}

impl Namespaces {
    pub fn apply_setns(&self) -> Result<()> {
        let to_enter: Vec<(CloneFlags, i32)> = self
            .spaces
            .iter()
            .filter(|ns| ns.path.is_some())
            .map(|ns| {
                let space = CloneFlags::from_bits_truncate(ns.typ as i32);
                let fd = fcntl::open(
                    &*ns.path.as_ref().unwrap().clone(),
                    fcntl::OFlag::empty(),
                    stat::Mode::empty(),
                )
                .unwrap();
                (space, fd)
            })
            .collect();
        for &(space, fd) in &to_enter {
            self.command.set_ns(fd, space)?;
            unistd::close(fd)?;
            if space == sched::CloneFlags::CLONE_NEWUSER {
                self.command.set_id(Uid::from_raw(0), Gid::from_raw(0))?;
            }
        }
        Ok(())
    }

    pub fn apply_unshare(&self, without: CloneFlags) -> Result<()> {
        sched::unshare(self.clone_flags & !without)?;
        Ok(())
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_namespaces_set_ns() {
        let namespaces: Namespaces = vec![LinuxNamespace {
            typ: LinuxNamespaceType::Mount,
            path: None,
        }]
        .into();
        assert_eq!(namespaces.apply_setns().is_ok(), true)
    }
}
