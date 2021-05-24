use std::string::ToString;

pub enum ControllerType {
    Devices,
    HugeTlb,
    Pids,
    Memory,
}

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Devices => "devices".into(),
            Self::HugeTlb => "hugetlb".into(),
            Self::Pids => "pids".into(),
            Self::Memory => "memory".into(),
        }
    }
}
