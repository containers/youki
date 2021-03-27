#!/bin/bash

go get github.com/opencontainers/runtime-tools
cd /workspaces/go/src/github.com/opencontainers/runtime-tools
make runtimetest validation-executables
if [ ! -e /workspaces/runtime-tools ]; then
    ln -s /workspaces/go/src/github.com/opencontainers/runtime-tools /workspaces
fi
# YOUKI_LOGLEVEL=debug RUNTIME=/workspaces/youki/target/x86_64-unknown-linux-gnu/debug/youki validation/kill/kill.t 
