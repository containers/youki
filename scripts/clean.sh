#!/usr/bin/env bash
set -euo pipefail

for bin in youki integration_test runtimetest test.log; do
    if [ -f $bin ]; then
        rm -f ${1}/$bin
    fi
done

rm -rf runtimetest-target

cargo clean

exit 0 # unconditionally return zero
