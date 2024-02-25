use super::ContainerData;
use anyhow::{bail, Result};
use libcgroups::v1::util::get_memory_data;
use oci_spec::runtime::Spec;

pub(crate) fn validate_linux_resource_memory(spec: &Spec, data: ContainerData) -> Result<()> {
    let expected_memory = spec
        .linux()
        .as_ref()
        .unwrap()
        .resources()
        .as_ref()
        .unwrap()
        .memory()
        .as_ref();
    let expected_memory = match expected_memory {
        Some(m) => m,
        None => bail!("expected memory to be set, but it was not"),
    };

    let memory = get_memory_data(data.state.unwrap().pid.unwrap());

    if memory.is_err() {
        bail!("failed to get memory data: {:?}", memory.err().unwrap());
    }

    if expected_memory.limit().unwrap() != memory.as_ref().unwrap().limit().unwrap() {
        bail!("expected memory {:?}, got {:?}", expected_memory, memory);
    }

    if expected_memory.swappiness().unwrap() != memory.as_ref().unwrap().swappiness().unwrap() {
        bail!("expected memory {:?}, got {:?}", expected_memory, memory);
    }

    Ok(())
}
