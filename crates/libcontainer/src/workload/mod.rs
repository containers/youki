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

// Here is an explanation about the complexity below regarding to
// CloneBoxExecutor and Executor traits. This is one of the places rust actually
// makes our life harder. The usecase for the executor is to allow users of
// `libcontainer` to pass in a closure like function where the actual execution
// of the container workload can be defined by user. To maximize the flexibility
// for the users, we use trait object to allow users to pass in a closure like
// objects, so the function can container any number of variables through the
// structure. This is similar to the Fn family of traits that rust std lib has.
// However, our usecase has a little bit more complexity than the Fn family of
// traits. We require the struct implementing this Executor traits to be
// cloneable, so we can pass the struct across fork/clone process boundary with
// memory safety. We can't make the Executor trait to require Clone trait
// because doing so will make the Executor trait not object safe. Part of the
// reason is that without the CloneBoxExecutor trait, the default clone
// implementation for Box<dyn trait> will first unwrap the box. However, the
// `dyn trait` inside the box doesn't have a size, which violates the object
// safety requirement for a trait. To work around this, we implement our own
// CloneBoxExecutor trait, which is object safe.
//
// Note to future maintainers: if you find a better way to do this or Rust
// introduced some new magical feature to simplify this logic, please consider
// to refactor this part.

pub trait CloneBoxExecutor {
    fn clone_box(&self) -> Box<dyn Executor>;
}

pub trait Executor: CloneBoxExecutor {
    /// Executes the workload
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError>;

    /// Validate if the spec can be executed by the executor. This step runs
    /// after the container init process is created, entered into the correct
    /// namespace and cgroups, and pivot_root into the rootfs. But this step
    /// runs before waiting for the container start signal.
    fn validate(&self, spec: &Spec) -> Result<(), ExecutorError>;
}

impl<T> CloneBoxExecutor for T
where
    T: 'static + Executor + Clone,
{
    fn clone_box(&self) -> Box<dyn Executor> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Executor> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
