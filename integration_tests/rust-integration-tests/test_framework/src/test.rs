///! Contains definition for a simple and commonly usable test structure
use crate::testable::{TestResult, Testable};

// type alias for the test function
type TestFn = dyn Sync + Send + Fn() -> TestResult;

/// Basic Template structure for a test
pub struct Test<'a> {
    /// name of the test
    name: &'a str,
    /// Actual test function
    test_fn: Box<TestFn>,
}

impl<'a> Test<'a> {
    /// create new test
    pub fn new(name: &'a str, test_fn: Box<TestFn>) -> Self {
        Test { name, test_fn }
    }
}

impl<'a> Testable<'a> for Test<'a> {
    fn get_name(&self) -> &'a str {
        self.name
    }

    fn run(&self) -> TestResult {
        (self.test_fn)()
    }
}
