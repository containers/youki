use std::string::ToString;

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

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Cpu => "cpu".into(),
            Self::CpuSet => "cpuset".into(),
            Self::Devices => "devices".into(),
            Self::HugeTlb => "hugetlb".into(),
            Self::Pids => "pids".into(),
            Self::Memory => "memory".into(),
            Self::Blkio => "blkio".into(),
            Self::NetworkPriority => "net_prio".into(),
            Self::NetworkClassifier => "net_cls".into(),
        }
    }
}
