#!/bin/bash
cd ../../
./build.sh --release
cp ./youki ./crates/integration_test/youki
cp ./youki_integration_test ./crates/integration_test/youki_integration_test
cp ./runtimetest_tool ./crates/integration_test/runtimetest
cd ./crates/integration_test

RUNTIME=./youki

# syntax is 
# test.sh build
# test.sh run
# test.sh run runtime-name

if [[ "$1" = "build" ]]; then
    exit 0
fi

# if second argument is non-empty, consider it as runtime name
# else the consider first argument as runtime name
if [[ -n "$2" ]]; then
    RUNTIME="$2"
elif [[-n "$1" ]]
    RUNTIME="$1"
fi


logfile="./test_log.log"
touch $logfile
sudo ./youki_integration_test run --runtime $RUNTIME --runtimetest ./runtimetest > $logfile
if [ 0 -ne $(grep "not ok" $logfile | wc -l ) ]; then
    cat $logfile
    exit 1
fi
echo "Validation successful for runtime $RUNTIME"