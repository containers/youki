#! /bin/sh -eu

ROOT=$(git rev-parse --show-toplevel)
RUNTIME=$1

if [ "$RUNTIME" = "" ]; then
    echo "please specify runtime"
    exit 1
fi

if [ ! -e $RUNTIME ]; then
  if ! which $RUNTIME ; then
    echo "$RUNTIME not found"
    exit 1
  fi
fi

ROOT=${2-$(git rev-parse --show-toplevel)}

LOGFILE="${ROOT}/test.log"

if [ ! -f ${ROOT}/bundle.tar.gz ]; then
    cp ${ROOT}/tests/integration_test/bundle.tar.gz ${ROOT}/bundle.tar.gz
fi
touch ${LOGFILE}

sudo ${ROOT}/integration_test run --runtime "$RUNTIME" --runtimetest ${ROOT}/runtimetest > $LOGFILE

if [ 0 -ne $(grep "not ok" $LOGFILE | wc -l ) ]; then
    cat $LOGFILE
    exit 1
fi

echo "Validation successful for runtime $1"
exit 0


