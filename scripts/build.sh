#! /bin/sh -eu

ROOT=$(git rev-parse --show-toplevel)
OUTPUT=${1:-$ROOT/bin}

[ ! -d $OUTPUT ] && mkdir -p $OUTPUT

cd ${ROOT}/crates
make release
mv ./youki_bin ${OUTPUT}/youki

cd ${ROOT}/tests/rust-integration-tests
make FLAG=--release all
mv ./runtimetest_bin ${OUTPUT}/runtimetest
mv ./integration_test_bin ${OUTPUT}/integration_test

exit 0
