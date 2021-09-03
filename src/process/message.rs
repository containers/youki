/// Used as a wrapper for messages to be sent between child and parent processes
#[derive(Debug)]
pub enum Message {
    IntermediateReady = 0x00,
    InitReady = 0x01,
    WriteMapping = 0x02,
    MappingWritten = 0x03,
}

impl From<u8> for Message {
    fn from(from: u8) -> Self {
        match from {
            0x00 => Message::IntermediateReady,
            0x01 => Message::InitReady,
            0x02 => Message::WriteMapping,
            0x03 => Message::MappingWritten,
            _ => panic!("unknown message."),
        }
    }
}
