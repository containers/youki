///! Contains structure for a test group
use crate::testable::{TestResult, Testable};
use std::collections::BTreeMap;

/// Stores tests belonging to a group
pub struct TestGroup {
    /// name of the test group
    name: String,
    /// tests belonging to this group
    tests: BTreeMap<String, Box<dyn Testable>>,
}

impl TestGroup {
    /// create a new test group
    pub fn new(name: &str) -> Self {
        TestGroup {
            name: name.to_string(),
            tests: BTreeMap::new(),
        }
    }

    /// get name of the test group
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// add a test to the group
    pub fn add(&mut self, tests: Vec<impl Testable + 'static>) {
        tests.into_iter().for_each(|t| {
            self.tests.insert(t.get_name(), Box::new(t));
        });
    }

    /// run all the test from the test group
    pub fn run_all(&self) -> Vec<(String, TestResult)> {
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
    pub fn run_selected(&self, selected: &[&str]) -> Vec<(String, TestResult)> {
        self.tests
            .iter()
            .filter(|(name, _)| selected.contains(&name.as_str()))
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
