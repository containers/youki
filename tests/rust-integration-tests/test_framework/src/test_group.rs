///! Contains structure for a test group
use crate::testable::{TestError, TestResult, Testable, TestableGroup};
use crossbeam::thread;
use std::collections::BTreeMap;

/// Stores tests belonging to a group
pub struct TestGroup {
    /// name of the test group
    name: &'static str,
    /// tests belonging to this group
    tests: BTreeMap<&'static str, Box<dyn Testable + Sync + Send>>,
}

impl TestGroup {
    /// create a new test group
    pub fn new(name: &'static str) -> Self {
        TestGroup {
            name,
            tests: BTreeMap::new(),
        }
    }

    /// add a test to the group
    pub fn add(&mut self, tests: Vec<Box<impl Testable + Sync + Send + 'static>>) {
        tests.into_iter().for_each(|t| {
            self.tests.insert(t.get_name(), t);
        });
    }

    pub fn with(mut self, test: Box<impl Testable + Sync + Send + 'static>) -> Self {
        self.tests.insert(test.get_name(), test);

        self
    }
}

impl TestableGroup for TestGroup {
    /// get name of the test group
    fn get_name(&self) -> &'static str {
        self.name
    }

    /// run all the test from the test group
    fn run_all(&self) -> Vec<(&'static str, TestResult<()>)> {
        let mut ret = Vec::with_capacity(self.tests.len());
        thread::scope(|s| {
            let mut collector = Vec::with_capacity(self.tests.len());
            for (_, t) in self.tests.iter() {
                let _t = s.spawn(move |_| {
                    if t.can_run() {
                        (t.get_name(), t.run())
                    } else {
                        (t.get_name(), Err(TestError::Skipped))
                    }
                });
                collector.push(_t);
            }
            for handle in collector {
                ret.push(handle.join().unwrap());
            }
        })
        .unwrap();
        ret
    }

    /// run selected test from the group
    fn run_selected(&self, selected: &[&str]) -> Vec<(&'static str, TestResult<()>)> {
        let selected_tests = self
            .tests
            .iter()
            .filter(|(name, _)| selected.contains(name));
        let mut ret = Vec::with_capacity(selected.len());
        thread::scope(|s| {
            let mut collector = Vec::with_capacity(selected.len());
            for (_, t) in selected_tests {
                let _t = s.spawn(move |_| {
                    if t.can_run() {
                        (t.get_name(), t.run())
                    } else {
                        (t.get_name(), Err(TestError::Skipped))
                    }
                });
                collector.push(_t);
            }
            for handle in collector {
                ret.push(handle.join().unwrap());
            }
        })
        .unwrap();
        ret
    }
}
