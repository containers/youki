#!/bin/bash

set -e

# syntax is 
# test.sh runtime-name

if test "$(id -u)" != 0; then
	echo "Please run as root"
	exit 1
fi

PROJECT_ROOT=$(git rev-parse --show-toplevel)
LOGFILE="$PROJECT_ROOT/crates/integration_test/test_log.log"
YOUKI_INTEGRATION_TEST=$PROJECT_ROOT/youki_integration_test
RUNTIMETEST_TOOL=$PROJECT_ROOT/runtimetest_tool

if [ ! -e $YOUKI_INTEGRATION_TEST ]; then
    echo "$YOUKI_INTEGRATION_TEST was not found, please try to build."
    exit 1
fi

if [ ! -e $RUNTIMETEST_TOOL ]; then
    echo "$RUNTIMETEST_TOOL was not found, please try to build."
    exit 1
fi

# if second argument is non-empty, consider it as runtime name
# else the consider first argument as runtime name
RUNTIME=$PROJECT_ROOT/youki
if [[ -n "$2" ]]; then
    RUNTIME=$2
elif [[ -n "$1" ]]; then
    RUNTIME=$1
fi

touch $LOGFILE

# FIXME: Tests should pass even if the log level is debug.
YOUKI_LOG_LEVEL="error" $YOUKI_INTEGRATION_TEST run --runtime $RUNTIME --runtimetest $RUNTIMETEST_TOOL > $LOGFILE
if [ 0 -ne $(grep "not ok" $LOGFILE | wc -l ) ]; then
    cat $LOGFILE
    exit 1
fi

echo "Validation successful for runtime $RUNTIME"
