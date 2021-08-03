use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// Windows defines the runtime configuration for Windows based containers, including Hyper-V
/// containers.
pub struct Windows {
    #[serde(rename = "layerFolders")]
    /// LayerFolders contains a list of absolute paths to directories containing image layers.
    pub layer_folders: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Devices are the list of devices to be mapped into the container.
    pub devices: Option<Vec<WindowsDevice>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Resources contains information for handling resource constraints for the container.
    pub resources: Option<WindowsResources>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "credentialSpec"
    )]
    /// CredentialSpec contains a JSON object describing a group Managed Service Account (gMSA)
    /// specification.
    pub credential_spec: Option<HashMap<String, Option<serde_json::Value>>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Servicing indicates if the container is being started in a mode to apply a Windows Update
    /// servicing operation.
    pub servicing: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "ignoreFlushesDuringBoot"
    )]
    /// IgnoreFlushesDuringBoot indicates if the container is being started in a mode where disk
    /// writes are not flushed during its boot process.
    pub ignore_flushes_during_boot: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// HyperV contains information for running a container with Hyper-V isolation.
    pub hyperv: Option<WindowsHyperV>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Network restriction configuration.
    pub network: Option<WindowsNetwork>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// WindowsDevice represents information about a host device to be mapped into the container.
pub struct WindowsDevice {
    /// Device identifier: interface class GUID, etc..
    pub id: String,

    #[serde(rename = "idType")]
    /// Device identifier type: "class", etc..
    pub id_type: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowsResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Memory restriction configuration.
    pub memory: Option<WindowsMemoryResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CPU resource restriction configuration.
    pub cpu: Option<WindowsCPUResources>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Storage restriction configuration.
    pub storage: Option<WindowsStorageResources>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// WindowsMemoryResources contains memory resource management settings.
pub struct WindowsMemoryResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Memory limit in bytes.
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// WindowsCPUResources contains CPU resource management settings.
pub struct WindowsCPUResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Number of CPUs available to the container.
    pub count: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// CPU shares (relative weight to other containers with cpu shares).
    pub shares: Option<u16>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies the portion of processor cycles that this container can use as a percentage times
    /// 100.
    pub maximum: Option<u16>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// WindowsStorageResources contains storage resource management settings.
pub struct WindowsStorageResources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies maximum Iops for the system drive.
    pub iops: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Specifies maximum bytes per second for the system drive.
    pub bps: Option<u64>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "sandboxSize"
    )]
    /// Sandbox size specifies the minimum size of the system drive in bytes.
    pub sandbox_size: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// WindowsHyperV contains information for configuring a container to run with Hyper-V isolation.
pub struct WindowsHyperV {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "utilityVMPath"
    )]
    /// UtilityVMPath is an optional path to the image used for the Utility VM.
    pub utility_vm_path: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// WindowsNetwork contains network settings for Windows containers.
pub struct WindowsNetwork {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "endpointList"
    )]
    /// List of HNS endpoints that the container should connect to.
    pub endpoint_list: Option<Vec<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "allowUnqualifiedDNSQuery"
    )]
    /// Specifies if unqualified DNS name resolution is allowed.
    pub allow_unqualified_dns_query: Option<bool>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "DNSSearchList"
    )]
    /// Comma separated list of DNS suffixes to use for name resolution.
    pub dns_search_list: Option<Vec<String>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "networkSharedContainerName"
    )]
    /// Name (ID) of the container that we will share with the network stack.
    pub network_shared_container_name: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "networkNamespace"
    )]
    /// name (ID) of the network namespace that will be used for the container.
    pub network_namespace: Option<String>,
}
