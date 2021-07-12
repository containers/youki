//! Handles Management of Capabilities
use crate::command::Syscall;
use caps::*;

use anyhow::Result;
use oci_spec::{LinuxCapabilities, LinuxCapabilityType};

/// Converts a list of capability types to capabilities has set
fn to_set(caps: &[LinuxCapabilityType]) -> CapsHashSet {
    let mut capabilities = CapsHashSet::new();
    for c in caps {
        capabilities.insert(c.cap);
    }
    capabilities
}

/// reset capabilities of process calling this to effective capabilities
/// effective capability set is set of capabilities used by kernel to perform checks
/// see https://man7.org/linux/man-pages/man7/capabilities.7.html for more information
pub fn reset_effective(syscall: &impl Syscall) -> Result<()> {
    log::debug!("reset all caps");
    syscall.set_capability(CapSet::Effective, &caps::all())?;
    Ok(())
}

/// Drop any extra granted capabilities, and reset to defaults which are in oci specification
pub fn drop_privileges(cs: &LinuxCapabilities, syscall: &impl Syscall) -> Result<()> {
    log::debug!("dropping bounding capabilities to {:?}", cs.bounding);
    syscall.set_capability(CapSet::Bounding, &to_set(&cs.bounding))?;

    syscall.set_capability(CapSet::Effective, &to_set(&cs.effective))?;
    syscall.set_capability(CapSet::Permitted, &to_set(&cs.permitted))?;
    syscall.set_capability(CapSet::Inheritable, &to_set(&cs.inheritable))?;

    // check specifically for ambient, as those might not always be available
    if let Err(e) = syscall.set_capability(CapSet::Ambient, &to_set(&cs.ambient)) {
        log::error!("failed to set ambient capabilities: {}", e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::test::TestHelperSyscall;

    #[test]
    fn test_reset_effective() {
        let test_command = TestHelperSyscall::default();
        assert!(reset_effective(&test_command).is_ok());
        let set_capability_args: Vec<_> = test_command
            .get_set_capability_args()
            .into_iter()
            .map(|(_capset, caps)| caps)
            .collect();
        assert_eq!(set_capability_args, vec![caps::all()]);
    }
}
