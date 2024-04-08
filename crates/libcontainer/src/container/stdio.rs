use crate::pipe::{PipeReader, PipeWriter};

pub struct StdioFds {
    pub stdin: Option<PipeWriter>,
    pub stdout: Option<PipeReader>,
    pub stderr: Option<PipeReader>,
}
