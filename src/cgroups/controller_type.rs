use std::string::ToString;

pub enum ControllerType {
    Devices,
    HugeTlb,
    Pids,
    Memory,
    Blkio,
    NetworkPriority,
    NetworkClassifier,
    Cpu,
}

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Devices => "devices".into(),
            Self::HugeTlb => "hugetlb".into(),
            Self::Cpu => "cpu".into(),
            Self::Pids => "pids".into(),
            Self::Memory => "memory".into(),
            Self::Blkio => "blkio".into(),
            Self::NetworkPriority => "net_prio".into(),
            Self::NetworkClassifier => "net_cls".into(),
        }
    }
}
