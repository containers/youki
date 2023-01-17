#!/bin/bash
set -eu

# Build the different features individually
cargo build --no-default-features -F v1
cargo build --no-default-features -F v2
cargo build --no-default-features -F systemd
cargo build --no-default-features -F v2 -F cgroupsv2_devices
cargo build --no-default-features -F systemd -F cgroupsv2_devices

# Test the different features individually
cargo test --no-default-features -F v1
cargo test --no-default-features -F v2
cargo test --no-default-features -F systemd
cargo test --no-default-features -F v2 -F cgroupsv2_devices
cargo test --no-default-features -F systemd -F cgroupsv2_devices

# Build with musl: libcontainer
cargo +nightly build \
    -Zbuild-std \
    --target $(uname -m)-unknown-linux-musl \
    --package libcontainer \
    --no-default-features -F v2

# Test with musl: libcontainer
cargo +nightly test \
    -Zbuild-std \
    --target $(uname -m)-unknown-linux-musl \
    --package libcontainer \
    --no-default-features -F v2

exit 0