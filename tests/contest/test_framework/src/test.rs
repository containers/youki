//! Contains definition for a simple and commonly usable test structure
use crate::testable::{TestResult, Testable};

// type alias for the test function
type TestFn = dyn Sync + Send + Fn() -> TestResult;

/// Basic Template structure for a test
pub struct Test {
    /// name of the test
    name: &'static str,
    /// Actual test function
    test_fn: Box<TestFn>,
}

impl Test {
    /// create new test
    pub fn new(name: &'static str, test_fn: Box<TestFn>) -> Self {
        Test { name, test_fn }
    }
}

impl Testable for Test {
    fn get_name(&self) -> &'static str {
        self.name
    }

    fn run(&self) -> TestResult {
        (self.test_fn)()
    }
}
