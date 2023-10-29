#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)

usage_exit() {
    echo "Usage: $0 [-r] [-o dir] [-c crate] [-a arch] [-f features]" 1>&2
    exit 1
}

VERSION=debug
CRATE="youki"
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
if [ "$ARCH" != "$(uname -m)" ]; then
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

if [ "$CRATE" == "contest" ]; then
    find ${OUTPUT} -maxdepth 1 -type f -name "contest" -exec rm -ifv {} \;
    cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin contest
    mv ${ROOT}/target/${TARGET}/${VERSION}/contest ${OUTPUT}/

    find ${OUTPUT} -maxdepth 1 -type f -name "runtimetest" -exec rm -ifv {} \;
    CONTEST_TARGET="$ROOT/contest-target"
    CARGO_TARGET_DIR=${CONTEST_TARGET} RUSTFLAGS="-Ctarget-feature=+crt-static" cargo build --target ${TARGET} ${OPTION} ${FEATURES} --bin runtimetest
    mv ${CONTEST_TARGET}/${TARGET}/${VERSION}/runtimetest ${OUTPUT}/
fi
