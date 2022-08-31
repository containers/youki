#!/bin/bash

set -e

ROOT=$(git rev-parse --show-toplevel)

usage_exit() {
    echo "Usage: $0 [-r] [-o dir]" 1>&2
    exit 1
}

VERSION=debug
TARGET=x86_64-unknown-linux-gnu
RUNTIMETEST_TARGET="$ROOT/runtimetest-target"
while getopts f:ro:h OPT; do
    case $OPT in
        f) features=${OPTARG}
            ;;
        o) output=${OPTARG}
            ;;
        r) VERSION=release
            ;;
        h) usage_exit
            ;;
        \?) usage_exit
            ;;
    esac
done

shift $((OPTIND - 1))

OPTION=""
if [ ${VERSION} = release ]; then
    OPTION="--${VERSION}"
fi

FEATURES=""
if [ -n "${features}" ]; then
    FEATURES="--features ${features}"
fi
echo "* FEATURES: ${FEATURES}"
echo "* features: ${features}"

OUTPUT=${output:-$ROOT/bin}
[ ! -d $OUTPUT ] && mkdir -p $OUTPUT

cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin youki
cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin integration_test
CARGO_TARGET_DIR=${RUNTIMETEST_TARGET} RUSTFLAGS="-Ctarget-feature=+crt-static" cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin runtimetest

mv ${ROOT}/target/${TARGET}/${VERSION}/{youki,integration_test} ${OUTPUT}/
mv ${RUNTIMETEST_TARGET}/${TARGET}/${VERSION}/runtimetest ${OUTPUT}/

exit 0
