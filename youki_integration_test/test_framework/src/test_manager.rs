///! This exposes the main control wrapper to control the tests
use crate::testable::{TestResult, TestableGroup};
use anyhow::Result;
use crossbeam::thread;
use std::collections::BTreeMap;

type TestableGroupType<'a> = dyn TestableGroup<'a> + Sync + Send + 'a;

/// This manages all test groups, and thus the tests
pub struct TestManager<'a> {
    test_groups: BTreeMap<&'a str, &'a TestableGroupType<'a>>,
    cleanup: Vec<Box<dyn Fn() -> Result<()>>>,
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
            cleanup: Vec::new(),
        }
    }

    /// add a test group to the test manager
    pub fn add_test_group(&mut self, tg: &'a TestableGroupType<'a>) {
        self.test_groups.insert(tg.get_name(), tg);
    }

    pub fn add_cleanup(&mut self, cleaner: Box<dyn Fn() -> Result<()>>) {
        self.cleanup.push(cleaner)
    }

    /// Prints the given test results, usually used to print
    /// results of a test group
    fn print_test_result(&self, name: &str, res: &[(&'a str, TestResult)]) {
        println!("# Start group {}", name);
        let len = res.len();
        for (idx, (name, res)) in res.iter().enumerate() {
            print!("{} / {} : {} : ", idx + 1, len, name);
            match res {
                TestResult::Passed => {
                    println!("ok");
                }
                TestResult::Skipped => {
                    println!("skipped");
                }
                TestResult::Failed(e) => {
                    println!("not ok\n\t{}", e);
                }
            }
        }
        println!("# End group {}\n", name);
    }
    /// Run all tests from all tests group
    pub fn run_all(&self) {
        thread::scope(|s| {
            let mut collector = Vec::with_capacity(self.test_groups.len());
            for (name, tg) in &self.test_groups {
                let r = s.spawn(move |_| tg.run_all());
                collector.push((name, r));
            }
            for (name, handle) in collector {
                self.print_test_result(name, &handle.join().unwrap());
            }
        })
        .unwrap();
        for cleaner in &self.cleanup {
            if let Err(e) = cleaner() {
                print!("Failed to cleanup: {}", e);
            }
        }
    }

    /// Run only selected tests
    pub fn run_selected(&self, tests: Vec<(&str, Option<Vec<&str>>)>) {
        thread::scope(|s| {
            let mut collector = Vec::with_capacity(tests.len());
            for (test_group_name, tests) in &tests {
                if let Some(tg) = self.test_groups.get(test_group_name) {
                    let r;
                    match tests {
                        None => r = s.spawn(move |_| tg.run_all()),
                        Some(tests) => r = s.spawn(move |_| tg.run_selected(tests)),
                    }
                    collector.push((test_group_name, r));
                } else {
                    eprintln!("Error : Test Group {} not found, skipping", test_group_name);
                }
            }
            for (name, handle) in collector {
                self.print_test_result(name, &handle.join().unwrap());
            }
        })
        .unwrap();

        for cleaner in &self.cleanup {
            if let Err(e) = cleaner() {
                print!("Failed to cleanup: {}", e);
            }
        }
    }
}
