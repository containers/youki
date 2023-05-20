use anyhow::Context;
use anyhow::Result;
use nix::sys::wait;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct TestError {
    source: Option<Box<TestError>>,
    description: String,
}

impl TestError {
    pub fn new<T>(e: &T) -> TestError
    where
        T: ?Sized + std::error::Error,
    {
        TestError {
            description: e.to_string(),
            source: e.source().map(|s| Box::new(TestError::new(s))),
        }
    }
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl std::error::Error for TestError {
    fn source(&self) -> Option<&(dyn 'static + std::error::Error)> {
        self.source
            .as_ref()
            .map(|source| &**source as &(dyn 'static + std::error::Error))
    }

    fn description(&self) -> &str {
        &self.description
    }
}

type TestResult = Result<(), TestError>;

pub fn test_in_child_process<F>(cb: F) -> Result<()>
where
    F: FnOnce() -> Result<()> + std::panic::UnwindSafe,
{
    let (mut sender, mut receiver) = crate::channel::channel::<TestResult>()?;
    match unsafe { nix::unistd::fork()? } {
        nix::unistd::ForkResult::Parent { child } => {
            let res = receiver.recv()?;
            wait::waitpid(child, None).with_context(|| "failed to wait for the child process")?;
            res.map_err(anyhow::Error::from)
                .with_context(|| "failed running function in the child process")?;
        }
        nix::unistd::ForkResult::Child => {
            let test_result = match std::panic::catch_unwind(cb) {
                Ok(ret) => ret.map_err(|err| TestError::new(&*err)),
                Err(err) => Err(TestError::new(&*anyhow::anyhow!(
                    "the child process paniced: {:?}",
                    err
                ))),
            };

            sender
                .send(test_result)
                .context("failed to send from the child process")?;
            std::process::exit(0);
        }
    };

    Ok(())
}

pub fn gen_u32() -> u32 {
    rand::thread_rng().gen()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, Result};

    #[test]
    fn test_child_process() -> Result<()> {
        test_in_child_process(|| Ok(()))?;
        if test_in_child_process(|| Err(anyhow::anyhow!("test error"))).is_ok() {
            bail!("expecting the child process to return an error")
        }

        Ok(())
    }

    #[test]
    fn test_panic_child_process() -> Result<()> {
        if test_in_child_process(|| {
            assert!(false, "this is a panic test");
            Ok(())
        })
        .is_ok()
        {
            bail!("expecting the child process to panic")
        }

        Ok(())
    }
}
