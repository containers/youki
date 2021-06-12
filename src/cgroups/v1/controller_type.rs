use std::fmt::Display;

pub enum ControllerType {
    Cpu,
    CpuSet,
    Devices,
    HugeTlb,
    Pids,
    Memory,
    Blkio,
    NetworkPriority,
    NetworkClassifier,
}

impl Display for ControllerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match *self {
            Self::Cpu => "cpu",
            Self::CpuSet => "cpuset",
            Self::Devices => "devices",
            Self::HugeTlb => "hugetlb",
            Self::Pids => "pids",
            Self::Memory => "memory",
            Self::Blkio => "blkio",
            Self::NetworkPriority => "net_prio",
            Self::NetworkClassifier => "net_cls",
        };

        write!(f, "{}", print)
    }
}

pub const CONTROLLERS: &[ControllerType] = &[
    ControllerType::Cpu,
    ControllerType::CpuSet,
    ControllerType::Devices,
    ControllerType::HugeTlb,
    ControllerType::Memory,
    ControllerType::Pids,
    ControllerType::Blkio,
    ControllerType::NetworkPriority,
    ControllerType::NetworkClassifier,
];
