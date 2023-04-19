use std::fmt::Debug;

///! Contains Basic setup for testing, testable trait and its result type
use anyhow::bail;

pub type TestResult<T> = std::result::Result<T, TestError>;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error("Test failed: {0}")]
    Failed(#[from] anyhow::Error),
    #[error("Test skipped")]
    Skipped,
}

/// This trait indicates that something can be run as a test, or is 'testable'
/// This forms the basis of the framework, as all places where tests are done,
/// expect structs which implement this
pub trait Testable {
    fn get_name(&self) -> &'static str;
    fn can_run(&self) -> bool {
        true
    }
    fn run(&self) -> TestResult<()>;
}

/// This trait indicates that something forms a group of tests.
/// Test groups are used to group tests in sensible manner as well as provide namespacing to tests
pub trait TestableGroup {
    fn get_name(&self) -> &'static str;
    fn run_all(&self) -> Vec<(&'static str, TestResult<()>)>;
    fn run_selected(&self, selected: &[&str]) -> Vec<(&'static str, TestResult<()>)>;
}

#[macro_export]
macro_rules! test_result {
    ($e:expr $(,)?) => {
        match $e {
            core::result::Result::Ok(val) => val,
            core::result::Result::Err(err) => {
                return Err($crate::testable::TestError::Failed(err.into()));
            }
        }
    };
}

#[macro_export]
macro_rules! assert_result_eq {
    ($expected:expr, $actual:expr $(,)?) => ({
        match (&$expected, &$actual) {
            (expected_val, actual_val) => {
                if !(*expected_val == *actual_val) {
                   test_framework::testable::assert_failed(&*expected_val, &*actual_val, std::option::Option::None)
                } else {
                    Ok(())
                }
            }
        }
    });
    ($expected:expr, $actual:expr, $($arg:tt)+) => ({
        match (&$expected, &$actual) {
            (expected_val, actual_val) => {
                if !(*expected_val == *actual_val) {
                    test_framework::testable::assert_failed(&*expected_val, &*actual_val, std::option::Option::Some(format_args!($($arg)+)))
                } else {
                    Ok(())
                }
            }
        }
    });
}

#[doc(hidden)]
pub fn assert_failed<T, U>(
    expected: &T,
    actual: &U,
    args: Option<std::fmt::Arguments<'_>>,
) -> anyhow::Result<()>
where
    T: Debug + ?Sized,
    U: Debug + ?Sized,
{
    match args {
        Some(args) => {
            bail!(
                r#"assertion failed:
            expected: `{:?}`,
            actual: `{:?}`: {}"#,
                expected,
                actual,
                args
            )
        }
        None => {
            bail!(
                r#"assertion failed:
            expected: `{:?}`,
            actual: `{:?}`"#,
                expected,
                actual
            )
        }
    }
}
