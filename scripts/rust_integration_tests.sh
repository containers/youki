#! /bin/sh -eu

ROOT=$(git rev-parse --show-toplevel)

if [ "$1" = "" ]; then
    echo "please specify runtime"
    exit 1
fi
ROOT=${2-$(git rev-parse --show-toplevel)}

LOGFILE="${ROOT}/test.log"

cd ${ROOT}

if [ ! -f ${ROOT}/bundle.tar.gz ]; then
    cp $(git rev-parse --show-toplevel)/tests/rust-integration-tests/integration_test/bundle.tar.gz ${ROOT}/bundle.tar.gz
fi
touch ${LOGFILE}

YOUKI_LOG_LEVEL="error" sudo ${ROOT}/integration_test run --runtime "$1" --runtimetest ./runtimetest > $LOGFILE

if [ 0 -ne $(grep "not ok" $LOGFILE | wc -l ) ]; then
    cat $LOGFILE
    exit 1
fi

echo "Validation successful for runtime $1"
exit 0


