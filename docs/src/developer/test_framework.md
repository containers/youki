# test_framework

This is the framework specifically developed to implement the ported integration tests. This exposes Structs to represent individual tests, test groups and test managers which runs the test groups. This also exposes a trait which can be used to implement a custom test struct or a custom test group.

By default the test groups are run in parallel using [crossbeam crate](https://www.crates.io/crates/crossbeam), and the default test_group implementation also runs individual tests parallelly. Sometimes you might need to run the individual test in certain order, serially such as when testing container lifecycle. In such cases you will need to implement the TestableGroup trait in a custom struct so you can finely control the order of execution.
