/// Used as a wrapper for messages to be sent between child and parent processes
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    IntermediateReady(i32),
    InitReady,
    WriteMapping,
    MappingWritten,
    SeccompNotify,
    SeccompNotifyDone,
}
