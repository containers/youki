#!/usr/bin/env bash
set -euo pipefail

test_package_features() {
    echo "[feature test] testing $1 with features $2"
    cargo build --no-default-features --package "$1" --features "$2"
}

test_package_features "libcontainer" "v1"
test_package_features "libcontainer" "v2"
test_package_features "libcontainer" "systemd"
test_package_features "libcontainer" "v2 cgroupsv2_devices"
test_package_features "libcontainer" "systemd cgroupsv2_devices"

test_package_features "libcgroups" "v1"
test_package_features "libcgroups" "v2"
test_package_features "libcgroups" "systemd"
test_package_features "libcgroups" "v2 cgroupsv2_devices"
test_package_features "libcgroups" "systemd cgroupsv2_devices"

test_features() {
    echo "[feature test] testing features $1"
    cargo build --no-default-features --features "$1"
    cargo test run --no-default-features --features "$1"
}

test_features "v1"
test_features "v2"
test_features "systemd"
test_features "v2 cgroupsv2_devices"
test_features "systemd cgroupsv2_devices"

exit 0