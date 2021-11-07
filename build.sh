#!/bin/bash

TARGET=${TARGET-x86_64-unknown-linux-gnu}
if [ "$TARGET" != "" ]; then
    TGT="--target $TARGET"
fi
VERSION=debug
if [[ "$1" == "--release" ]]; then
    VERSION=release
fi

cargo build --verbose $TGT $1
if [ ! -e ./youki ]; then
    rm -f youki
fi 
if [ ! -e ./youki_integration_test ]; then
    rm -f youki_integration_test
fi
cp target/$TARGET/$VERSION/youki .
cp target/$TARGET/$VERSION/integration_test ./youki_integration_test
