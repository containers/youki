pub enum ControllerType {
    Cpu,
    CpuSet,
    Io,
    Memory,
    HugeTlb,
    Pids,
    Freezer,
    Devices,
}

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Cpu => "cpu".into(),
            Self::CpuSet => "cpuset".into(),
            Self::Io => "io".into(),
            Self::Memory => "memory".into(),
            Self::HugeTlb => "hugetlb".into(),
            Self::Pids => "pids".into(),
            Self::Freezer => "freezer".into(),
            Self::Devices => "devices".into(),
        }
    }
}
