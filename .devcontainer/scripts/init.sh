#!/bin/bash

RUST_BACKTRACE=full YOUKI_LOG_LEVEL=debug YOUKI_MODE=/var/lib/docker/containers/ dockerd --experimental --add-runtime="youki=/workspaces/youki/target/x86_64-unknown-linux-gnu/debug/youki" &
cargo build
