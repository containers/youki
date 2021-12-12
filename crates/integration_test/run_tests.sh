#!/bin/bash
cd ../../
./build.sh --release
cp ./youki ./crates/integration_test
cp ./youki_integration_test ./crates/integration_test
cd ./crates/integration_test
RUNTIME=./youki
if [[ -n "$1" ]]; then
    RUNTIME="$1"
fi
logfile="./test_log.log"
touch $logfile
sudo ./youki_integration_test -r $RUNTIME > $logfile
if [ 0 -ne $(grep "not ok" $logfile | wc -l ) ]; then
    cat $logfile
    exit 1
fi
echo "Validation successful for runtime $RUNTIME"