///! Contains structure for a test group
use crate::testable::{TestResult, Testable, TestableGroup};
use std::collections::BTreeMap;

/// Stores tests belonging to a group
pub struct TestGroup<'a> {
    /// name of the test group
    name: &'a str,
    /// tests belonging to this group
    tests: BTreeMap<&'a str, Box<dyn Testable<'a> + Sync + Send + 'a>>,
}

impl<'a> TestGroup<'a> {
    /// create a new test group
    pub fn new(name: &'a str) -> Self {
        TestGroup {
            name,
            tests: BTreeMap::new(),
        }
    }

    /// add a test to the group
    pub fn add(&mut self, tests: Vec<Box<impl Testable<'a> + Sync + Send + 'a>>) {
        tests.into_iter().for_each(|t| {
            self.tests.insert(t.get_name(), t);
        });
    }
}

impl<'a> TestableGroup<'a> for TestGroup<'a> {
    /// get name of the test group
    fn get_name(&self) -> &'a str {
        self.name
    }

    /// run all the test from the test group
    fn run_all(&'a self) -> Vec<(&'a str, TestResult)> {
        self.tests
            .iter()
            .map(|(_, t)| {
                if t.can_run() {
                    (t.get_name(), t.run())
                } else {
                    (t.get_name(), TestResult::Skip)
                }
            })
            .collect()
    }

    /// run selected test from the group
    fn run_selected(&'a self, selected: &[&str]) -> Vec<(&'a str, TestResult)> {
        self.tests
            .iter()
            .filter(|(name, _)| selected.contains(name))
            .map(|(_, t)| {
                if t.can_run() {
                    (t.get_name(), t.run())
                } else {
                    (t.get_name(), TestResult::Skip)
                }
            })
            .collect()
    }
}
