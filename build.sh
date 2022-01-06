#!/bin/bash

set -e

TARGET=${TARGET-x86_64-unknown-linux-gnu}
if [ "$TARGET" != "" ]; then
    TGT="--target $TARGET"
fi
VERSION=debug
if [ "$1" == "--release" ]; then
    VERSION=release
fi

# We have to build the three binaries seprately for the following reason :
# The runtimetest MUST be compiled from its own directory, if compiled from root,
# it will not work as intended to test the runtime from inside
# So we just compile all thre binaries separately.
# To see why runtime test must be compiled in its own directory, see its Readme or its docs

cargo build --bin youki --verbose $TGT $1
cargo build --bin integration_test --verbose $TGT $1
cd crates/runtimetest
cargo build --verbose $TGT $1
cd ../../

cp target/$TARGET/$VERSION/youki .
cp target/$TARGET/$VERSION/integration_test ./youki_integration_test
cp target/$TARGET/$VERSION/runtimetest ./runtimetest
