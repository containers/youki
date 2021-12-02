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

cargo build --verbose $TGT $1

cp target/$TARGET/$VERSION/youki .
cp target/$TARGET/$VERSION/integration_test ./youki_integration_test
