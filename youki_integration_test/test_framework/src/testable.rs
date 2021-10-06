///! Contains Basic setup for testing, testable trait and its result type
use anyhow::{Error, Result};

#[derive(Debug)]
/// Enum indicating result of the test. This is like an extended std::result,
/// which includes a Skip variant to indicate that a test was skipped, and the Ok variant has no associated value
pub enum TestResult {
    /// Test was ok
    Ok,
    /// Test needed to be skipped
    Skip,
    /// Test was error
    Err(Error),
}

impl<T> From<Result<T>> for TestResult {
    fn from(result: Result<T>) -> Self {
        match result {
            Ok(_) => TestResult::Ok,
            Err(err) => TestResult::Err(err),
        }
    }
}

/// This trait indicates that something can be run as a test, or is 'testable'
/// This forms the basis of the framework, as all places where tests are done,
/// expect structs which implement this
pub trait Testable<'a> {
    fn get_name(&self) -> &'a str;
    fn can_run(&self) -> bool {
        true
    }
    fn run(&self) -> TestResult;
}

/// This trait indicates that something forms a group of tests.
/// Test groups are used to group tests in sensible manner as well as provide namespacing to tests
pub trait TestableGroup<'a> {
    fn get_name(&self) -> &'a str;
    fn run_all(&'a self) -> Vec<(&'a str, TestResult)>;
    fn run_selected(&'a self, selected: &[&str]) -> Vec<(&'a str, TestResult)>;
}
