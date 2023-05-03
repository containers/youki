use std::fmt::Display;

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
pub enum ControllerType {
    Cpu,
    CpuAcct,
    CpuSet,
    Devices,
    HugeTlb,
    Pids,
    PerfEvent,
    Memory,
    Blkio,
    NetworkPriority,
    NetworkClassifier,
    Freezer,
}

impl Display for ControllerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match *self {
            Self::Cpu => "cpu",
            Self::CpuAcct => "cpuacct",
            Self::CpuSet => "cpuset",
            Self::Devices => "devices",
            Self::HugeTlb => "hugetlb",
            Self::Pids => "pids",
            Self::PerfEvent => "perf_event",
            Self::Memory => "memory",
            Self::Blkio => "blkio",
            Self::NetworkPriority => "net_prio",
            Self::NetworkClassifier => "net_cls",
            Self::Freezer => "freezer",
        };

        write!(f, "{print}")
    }
}

impl AsRef<str> for ControllerType {
    fn as_ref(&self) -> &str {
        match *self {
            Self::Cpu => "cpu",
            Self::CpuAcct => "cpuacct",
            Self::CpuSet => "cpuset",
            Self::Devices => "devices",
            Self::HugeTlb => "hugetlb",
            Self::Pids => "pids",
            Self::PerfEvent => "perf_event",
            Self::Memory => "memory",
            Self::Blkio => "blkio",
            Self::NetworkPriority => "net_prio",
            Self::NetworkClassifier => "net_cls",
            Self::Freezer => "freezer",
        }
    }
}

pub const CONTROLLERS: &[ControllerType] = &[
    ControllerType::Cpu,
    ControllerType::CpuAcct,
    ControllerType::CpuSet,
    ControllerType::Devices,
    ControllerType::HugeTlb,
    ControllerType::Memory,
    ControllerType::Pids,
    ControllerType::PerfEvent,
    ControllerType::Blkio,
    ControllerType::NetworkPriority,
    ControllerType::NetworkClassifier,
    ControllerType::Freezer,
];
