#!/bin/bash
set -e

PROJECT_ROOT=$(git rev-parse --show-toplevel)
RUNTIME=${1:-"$PROJECT_ROOT/youki"}

cd $PROJECT_ROOT/tests/integration_test

TARGET=${TARGET-x86_64-unknown-linux-gnu}
if [ "$TARGET" != "" ]; then
    TGT="--target $TARGET"
fi
VERSION=debug
if [ "$1" == "--release" ]; then
    VERSION=release
fi
cargo build --verbose $TGT
cp $PROJECT_ROOT/tests/integration_test/target/$TARGET/$VERSION/integration_test ./youki_integration_test

logfile="./test_log.log"
touch $logfile
sudo ./youki_integration_test -r $RUNTIME > $logfile
if [ 0 -ne $(grep "not ok" $logfile | wc -l ) ]; then
    cat $logfile
    exit 1
fi
echo "Validation successful for runtime $RUNTIME"
