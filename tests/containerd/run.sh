#!/bin/bash

cd $1

if test "$(id -u)" != 0; then
	echo "run as root"
	exit 1
fi

cd /youki && ./build.sh --release && cp youki /usr/bin/runc
ulimit -u unlimited
export PATH=$PATH:$(pwd)/bin
make RUNC_FLAVOR=youki TEST_RUNTIME=io.containerd.runc.v2 TESTFLAGS="-timeout 120m" integration