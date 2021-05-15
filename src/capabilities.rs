use crate::{
    command::Command,
    spec::{LinuxCapabilities, LinuxCapabilityType},
};
use caps::*;

use anyhow::Result;

fn to_set(caps: &[LinuxCapabilityType]) -> CapsHashSet {
    let mut capabilities = CapsHashSet::new();
    for c in caps {
        capabilities.insert(c.cap);
    }
    capabilities
}

pub fn reset_effective(command: &impl Command) -> Result<()> {
    log::debug!("reset all caps");
    command.set_capability(CapSet::Effective, &caps::all())?;
    Ok(())
}

pub fn drop_privileges(cs: &LinuxCapabilities, command: &impl Command) -> Result<()> {
    let all = caps::all();
    log::debug!("dropping bounding capabilities to {:?}", cs.bounding);
    for c in all.difference(&to_set(&cs.bounding)) {
        match c {
            Capability::CAP_PERFMON | Capability::CAP_CHECKPOINT_RESTORE | Capability::CAP_BPF => {
                log::warn!("{:?} doesn't support.", c);
                continue;
            }
            _ => caps::drop(None, CapSet::Bounding, *c)?,
        }
    }

    command.set_capability(CapSet::Effective, &to_set(&cs.effective))?;
    command.set_capability(CapSet::Permitted, &to_set(&cs.permitted))?;
    command.set_capability(CapSet::Inheritable, &to_set(&cs.inheritable))?;

    if let Err(e) = command.set_capability(CapSet::Ambient, &to_set(&cs.ambient)) {
        log::error!("failed to set ambient capabilities: {}", e);
    }
    Ok(())
}

mod tests {
    use super::*;
    use crate::command::test::TestHelperCommand;

    #[test]
    fn test_reset_effective() {
        let test_command = TestHelperCommand::default();
        assert!(reset_effective(&test_command).is_ok());
        let set_capability_args: Vec<_> = test_command
            .get_set_capability_args()
            .into_iter()
            .map(|(_capset, caps)| caps)
            .collect();
        assert_eq!(set_capability_args, vec![caps::all()]);
    }
}
