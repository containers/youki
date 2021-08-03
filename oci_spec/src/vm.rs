use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// VM contains information for virtual-machine-based containers.
pub struct VM {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Hypervisor specifies hypervisor-related configuration for virtual-machine-based containers.
    pub hypervisor: Option<VMHypervisor>,

    /// Kernel specifies kernel-related configuration for virtual-machine-based containers.
    pub kernel: VMKernel,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Image specifies guest image related configuration for virtual-machine-based containers.
    pub image: Option<VMImage>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// VMHypervisor contains information about the hypervisor to use for a virtual machine.
pub struct VMHypervisor {
    /// Path is the host path to the hypervisor used to manage the virtual machine.
    pub path: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Parameters specifies parameters to pass to the hypervisor.
    pub parameters: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// VMKernel contains information about the kernel to use for a virtual machine.
pub struct VMKernel {
    /// Path is the host path to the kernel used to boot the virtual machine.
    pub path: PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Parameters specifies parameters to pass to the kernel.
    pub parameters: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// InitRD is the host path to an initial ramdisk to be used by the kernel.
    pub initrd: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
/// VMImage contains information about the virtual machine root image.
pub struct VMImage {
    /// Path is the host path to the root image that the VM kernel would boot into.
    pub path: PathBuf,

    /// Format is the root image format type (e.g. "qcow2", "raw", "vhd", etc).
    pub format: String,
}
