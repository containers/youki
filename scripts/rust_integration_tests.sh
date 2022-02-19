#! /bin/sh -eu

ROOT=$(git rev-parse --show-toplevel)

SCRIPT_DIR=${ROOT}/scripts

if [ "$1" = "" ]; then
    echo "please specify runtime"
    exit 1
fi

LOGFILE="${SCRIPT_DIR}/test.log"

cd ${SCRIPT_DIR}

${SCRIPT_DIR}/build.sh

cp ${ROOT}/integration_tests/rust-integration-tests/integration_test/bundle.tar.gz ${SCRIPT_DIR}/bundle.tar.gz
touch ${LOGFILE}

YOUKI_LOG_LEVEL="error" sudo ./integration_test run --runtime "$1" --runtimetest ./runtimetest > $LOGFILE

# remove the files copied
./clean.sh

if [ 0 -ne $(grep "not ok" $LOGFILE | wc -l ) ]; then
    cat $LOGFILE
    exit 1
fi

echo "Validation successful for runtime $1"
exit 0


