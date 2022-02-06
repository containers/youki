#! /bin/sh

cd ../crates
make release
mv ./youki_bin ../scripts/youki

cd ../integration_tests/rust-integration-tests
make FLAG=--release all
mv ./runtimetest_bin ../../scripts/runtimetest
mv ./integration_test_bin ../../scripts/integration_test

exit 0