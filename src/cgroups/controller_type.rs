use std::string::ToString;

pub enum ControllerType {
    Devices,
}

impl ToString for ControllerType {
    fn to_string(&self) -> String {
        match self {
            Self::Devices => "devices".into(),
        }
    }
}
