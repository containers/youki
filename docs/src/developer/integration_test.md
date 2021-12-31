# integration_test

This crate contains the Rust port of OCI-runtime tools integration tests, which are used to test if the runtime works as per the OCI spec or not. Initially youki used the original implementation of these test provided in the OCI repository [here](https://github.com/opencontainers/runtime-tools/tree/master/validation). But those tests are written in Go, which made the developers depend on two language environments Rust and Go to compile youki and test it. The Validation tests themselves also have an optional dependency on node js to parse their output, which can make it a third language dependency.

Other than that, those tests also showed some issues while running on some local systems, and thus running the tests would be difficult on local system. As the runtime is a complex piece of software, it becomes useful to have a set of tests that can be run with changes in code, so one can verify that change in one part of youki has not accidentally broken some other part of youki.

Thus we decided to port the tests to Rust, and validate them, so that we have a set of unit tests as well of integration tests to validate the working of runtime. These tests are still under development, and you can check the [tracking issue](https://github.com/containers/youki/issues/361) for more details. More details on working of these tests can be found at [https://github.com/containers/youki/tree/main/crates/integration_test](https://github.com/containers/youki/tree/main/crates/integration_test).

As these tests are under development, these are validated on a standard runtime such as runc in the GitHub CI, so validate the tests themselves.
