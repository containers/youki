#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)

usage_exit() {
    echo "Usage: $0 [-r] [-o dir]" 1>&2
    exit 1
}

VERSION=debug
TARGET="$(uname -m)-unknown-linux-gnu"
CRATE="youki"
RUNTIMETEST_TARGET="$ROOT/runtimetest-target"
features=""
while getopts f:ro:c:h OPT; do
    case $OPT in
        f) features=${OPTARG}
            ;;
        o) output=${OPTARG}
            ;;
        r) VERSION=release
            ;;
        c) CRATE=${OPTARG}
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


if [ "$CRATE" == "youki" ]; then
    rm -f ${OUTPUT}/youki
    cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin youki
    mv ${ROOT}/target/${TARGET}/${VERSION}/youki ${OUTPUT}/
fi

if [ "$CRATE" == "integration-test" ]; then
    rm -f ${OUTPUT}/integration_test
    cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin integration_test
    mv ${ROOT}/target/${TARGET}/${VERSION}/integration_test ${OUTPUT}/
fi

if [ "$CRATE" == "runtimetest" ]; then
    rm -f ${OUTPUT}/runtimetest
    CARGO_TARGET_DIR=${RUNTIMETEST_TARGET} RUSTFLAGS="-Ctarget-feature=+crt-static" cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin runtimetest
    mv ${RUNTIMETEST_TARGET}/${TARGET}/${VERSION}/runtimetest ${OUTPUT}/
fi

exit 0
