/// Used as a wrapper for messages to be sent between child and parent processes
#[derive(Debug)]
pub enum Message {
    ChildReady = 0x00,
    WriteMapping = 0x01,
    MappingWritten = 0x02,
}

impl From<u8> for Message {
    fn from(from: u8) -> Self {
        match from {
            0x00 => Message::ChildReady,
            0x01 => Message::WriteMapping,
            0x02 => Message::MappingWritten,
            _ => panic!("unknown message."),
        }
    }
}
