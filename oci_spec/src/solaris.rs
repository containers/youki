use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Solaris contains platform-specific configuration for Solaris application containers.
pub struct Solaris {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// SMF FMRI which should go "online" before we start the container process.
    pub milestone: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Maximum set of privileges any process in this container can obtain.
    pub limitpriv: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The maximum amount of shared memory allowed for this container.
    pub max_shm_memory: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specification for automatic creation of network resources for this container.
    pub anet: Option<Vec<SolarisAnet>>,

    #[serde(default, skip_serializing_if = "Option::is_none", rename = "cappedCPU")]
    /// Set limit on the amount of CPU time that can be used by container.
    pub capped_cpu: Option<SolarisCappedCPU>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The physical and swap caps on the memory that can be used by this container.
    pub capped_memory: Option<SolarisCappedMemory>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// SolarisAnet provides the specification for automatic creation of network resources for this
/// container.
pub struct SolarisAnet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specify a name for the automatically created VNIC datalink.
    pub linkname: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specify the link over which the VNIC will be created.
    pub lower_link: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The set of IP addresses that the container can use.
    pub allowed_address: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies whether allowedAddress limitation is to be applied to the VNIC.
    pub configure_allowed_address: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The value of the optional default router.
    pub defrouter: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Enable one or more types of link protection.
    pub link_protection: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Set the VNIC's macAddress.
    pub mac_address: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// SolarisCappedCPU allows users to set limit on the amount of CPU time that can be used by
/// container.
pub struct SolarisCappedCPU {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ncpus: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// SolarisCappedMemory allows users to set the physical and swap caps on the memory that can be
/// used by this container.
pub struct SolarisCappedMemory {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The physical caps on the memory.
    pub physical: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The swap caps on the memory.
    pub swap: Option<String>,
}
