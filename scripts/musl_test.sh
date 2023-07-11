#!/usr/bin/env bash
set -euo pipefail

test_musl() {
    echo "[musl test] testing $1 with features $2"
    cargo +nightly build \
        -Zbuild-std \
        --target $(uname -m)-unknown-linux-musl \
        --package "$1" \
        --no-default-features -F "$2"
    cargo +nightly test \
        -Zbuild-std \
        --target $(uname -m)-unknown-linux-musl \
        --package "$1" \
        --no-default-features -F "$2"
}

test_musl "libcontainer" "v1"
test_musl "libcontainer" "v2"
test_musl "libcontainer" "v1 v2"

