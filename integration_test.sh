#!/bin/bash -eu

root=$(pwd)
cd integration_test/src/github.com/opencontainers/runtime-tools
GOPATH=$root/integration_test make runtimetest validation-executables
test_cases=("default/default.t" "linux_cgroups_devices/linux_cgroups_devices.t" "linux_cgroups_hugetlb/linux_cgroups_hugetlb.t" "linux_cgroups_pids/linux_cgroups_pids.t" "linux_cgroups_memory/linux_cgroups_memory.t" "linux_cgroups_network/linux_cgroups_network.t")
for case in "${test_cases[@]}"; do
  echo "Running $case"
  if [ 0 -ne $(sudo RUST_BACKTRACE=1 RUNTIME=$root/target/x86_64-unknown-linux-gnu/debug/youki $root/integration_test/src/github.com/opencontainers/runtime-tools/validation/$case | grep "not ok" | wc -l) ]; then
      exit 1
  fi
done
