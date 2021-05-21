use std::string::ToString;

pub enum ControllerType {
    Devices,
    HugeTlb,
}

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Devices => "devices".into(),
            Self::HugeTlb => "hugetlb".into(),
        }
    }
}
