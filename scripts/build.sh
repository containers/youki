#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)

usage_exit() {
    echo "Usage: $0 [-r] [-o dir] [-c crate] [-a arch] [-f features]" 1>&2
    exit 1
}

VERSION=debug
CRATE="youki"
RUNTIMETEST_TARGET="$ROOT/runtimetest-target"
features=""
ARCH=$(uname -m)
while getopts f:ro:c:ha: OPT; do
    case $OPT in
        f) features=${OPTARG}
            ;;
        o) output=${OPTARG}
            ;;
        r) VERSION=release
            ;;
        c) CRATE=${OPTARG}
            ;;
        a) ARCH=${OPTARG}
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

TARGET="${ARCH}-unknown-linux-gnu"
CARGO="cargo"
if [ "$ARCH" == "aarch64" ]; then
  # shellcheck disable=SC2034
  CARGO="cross"
fi

FEATURES=""
if [ -n "${features}" ]; then
    FEATURES="--features ${features}"
fi
echo "* FEATURES: ${FEATURES}"
echo "* features: ${features}"
echo "* TARGET: ${TARGET}"

OUTPUT=${output:-$ROOT/bin}
[ ! -d $OUTPUT ] && mkdir -p $OUTPUT


if [ "$CRATE" == "youki" ]; then
    rm -f ${OUTPUT}/youki
    $CARGO build --target ${TARGET} ${OPTION} ${FEATURES} --bin youki
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
