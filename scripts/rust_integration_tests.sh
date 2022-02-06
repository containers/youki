#! /bin/sh

if [ "$RUNTIME" = "" ]; then
    RUNTIME="./youki"
fi

LOGFILE="./test.log"

./build.sh

cp ../integration_tests/rust-integration-tests/integration_test/bundle.tar.gz ./bundle.tar.gz
touch ./test.log

YOUKI_LOG_LEVEL="error" sudo ./integration_test run --runtime $RUNTIME --runtimetest ./runtimetest > $LOGFILE

# remove the files copied
./clean.sh

if [ 0 -ne $(grep "not ok" $LOGFILE | wc -l ) ]; then
    cat $LOGFILE
    exit 1
fi

echo "Validation successful for runtime $RUNTIME"
exit 0


