use std::fmt::Display;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControllerType {
    Cpu,
    CpuSet,
    Io,
    Memory,
    HugeTlb,
    Pids,
}

impl Display for ControllerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match self {
            Self::Cpu => "cpu",
            Self::CpuSet => "cpuset",
            Self::Io => "io",
            Self::Memory => "memory",
            Self::HugeTlb => "hugetlb",
            Self::Pids => "pids",
        };

        write!(f, "{print}")
    }
}

pub const CONTROLLER_TYPES: &[ControllerType] = &[
    ControllerType::Cpu,
    ControllerType::CpuSet,
    ControllerType::HugeTlb,
    ControllerType::Io,
    ControllerType::Memory,
    ControllerType::Pids,
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PseudoControllerType {
    Devices,
    Freezer,
    Unified,
}

impl Display for PseudoControllerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match self {
            Self::Devices => "devices",
            Self::Freezer => "freezer",
            Self::Unified => "unified",
        };

        write!(f, "{print}")
    }
}

pub const PSEUDO_CONTROLLER_TYPES: &[PseudoControllerType] = &[
    PseudoControllerType::Devices,
    PseudoControllerType::Freezer,
    PseudoControllerType::Unified,
];
