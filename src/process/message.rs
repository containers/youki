#[derive(Debug)]
pub enum Message {
    ChildReady = 0x00,
    InitReady = 0x01,
}

impl From<u8> for Message {
    fn from(from: u8) -> Self {
        match from {
            0x00 => Message::ChildReady,
            0x01 => Message::InitReady,
            _ => panic!("unknown message."),
        }
    }
}
