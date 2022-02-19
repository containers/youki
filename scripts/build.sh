#! /bin/sh -eu

ROOT=$(git rev-parse --show-toplevel)

cd ${ROOT}/crates
make release
mv ./youki_bin ${ROOT}/scripts/youki

cd ${ROOT}/integration_tests/rust-integration-tests
make FLAG=--release all
mv ./runtimetest_bin ${ROOT}/scripts/runtimetest
mv ./integration_test_bin ${ROOT}/scripts/integration_test

exit 0