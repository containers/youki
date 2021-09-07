///! This exposes the main control wrapper to control the tests
use crate::testable::{TestResult, TestableGroup};
use std::collections::BTreeMap;

/// This manages all test groups, and thus the tests
pub struct TestManager<'a> {
    test_groups: BTreeMap<String, &'a dyn TestableGroup>,
}

impl<'a> Default for TestManager<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> TestManager<'a> {
    /// Create new TestManager
    pub fn new() -> Self {
        TestManager {
            test_groups: BTreeMap::new(),
        }
    }

    /// add a test group to the test manager
    pub fn add_test_group(&mut self, tg: &'a dyn TestableGroup) {
        self.test_groups.insert(tg.get_name(), tg);
    }

    /// Prints the given test results, usually used to print
    /// results of a test group
    fn print_test_result(&self, name: &str, res: Vec<(&String, &TestResult)>) {
        println!("# Start group {}", name);
        let len = res.len();
        for (idx, (name, res)) in res.iter().enumerate() {
            print!("{} / {} : {} : ", idx + 1, len, name);
            match res {
                TestResult::Ok => {
                    println!("ok");
                }
                TestResult::Skip => {
                    println!("skipped");
                }
                TestResult::Err(e) => {
                    println!("not ok\n\t{}", e);
                }
            }
        }
        println!("\n# End group {}", name);
    }

    /// Run all tests from given group
    fn run_test_group(&self, name: &str, tg: &'a dyn TestableGroup) {
        let results = tg.run_all();
        let mut test_vec = Vec::new();
        for (name, res) in results.iter() {
            test_vec.push((name, res));
        }
        self.print_test_result(name, test_vec);
    }

    /// Run all tests from all tests group
    pub fn run_all(&self) {
        for (name, tg) in self.test_groups.iter() {
            self.run_test_group(name, *tg);
        }
    }

    /// Run only selected tests
    pub fn run_selected(&self, tests: Vec<(String, Option<Vec<&str>>)>) {
        for (test_group_name, tests) in tests.iter() {
            if let Some(tg) = self.test_groups.get(test_group_name) {
                match tests {
                    None => self.run_test_group(test_group_name, *tg),
                    Some(tests) => {
                        let results = tg.run_selected(tests);
                        let mut test_vec = Vec::new();
                        for (name, res) in results.iter() {
                            test_vec.push((name, res));
                        }
                        self.print_test_result(test_group_name, test_vec);
                    }
                }
            } else {
                eprintln!("Error : Test Group {} not found, skipping", test_group_name);
            }
        }
    }
}
