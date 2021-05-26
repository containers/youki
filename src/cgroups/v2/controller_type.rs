pub enum ControllerType {
    Cpu,
    CpuSet,
    IO,
    Memory,
    HugeTlb,
    Pids,
}

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Cpu => "cpu".into(),
            Self::CpuSet => "cpuset".into(),
            Self::IO => "io".into(),
            Self::Memory => "memory".into(),
            Self::HugeTlb => "hugetlb".into(),
            Self::Pids => "pids".into(),
        }
    }
}