use oci_spec::runtime::Spec;

pub mod default;

pub static EMPTY: Vec<String> = Vec::new();

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("invalid argument")]
    InvalidArg,
    #[error("failed to execute workload")]
    Execution(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("{0}")]
    Other(String),
    #[error("{0} executor can't handle spec")]
    CantHandle(&'static str),
}

pub type Executor = Box<fn(&Spec) -> Result<(), ExecutorError>>;
