use crate::spec::{LinuxCapabilities, LinuxCapabilityType};
use caps::*;

use anyhow::Result;

fn to_set(caps: &[LinuxCapabilityType]) -> CapsHashSet {
    let mut capabilities = CapsHashSet::new();
    for c in caps {
        capabilities.insert(c.cap);
    }
    capabilities
}

pub fn reset_effective() -> Result<()> {
    log::debug!("reset all caps");
    set(None, CapSet::Effective, &caps::all())?;
    Ok(())
}

pub fn drop_privileges(cs: &LinuxCapabilities) -> Result<()> {
    let all = caps::all();
    log::debug!("dropping bounding capabilities to {:?}", cs.bounding);
    for c in all.difference(&to_set(&cs.bounding)) {
        match c {
            Capability::CAP_PERFMON | Capability::CAP_CHECKPOINT_RESTORE | Capability::CAP_BPF => {
                continue
            }
            _ => caps::drop(None, CapSet::Bounding, *c)?,
        }
    }

    set(None, CapSet::Effective, &to_set(&cs.effective))?;
    set(None, CapSet::Permitted, &to_set(&cs.permitted))?;
    set(None, CapSet::Inheritable, &to_set(&cs.inheritable))?;

    if let Err(e) = set(None, CapSet::Ambient, &to_set(&cs.ambient)) {
        log::error!("failed to set ambient capabilities: {}", e);
    }
    Ok(())
}
