#!/bin/bash -eu

ROOT=$(pwd)
RUNTIME=${ROOT}/youki
PATTERN=${1:-.}

cd integration_test/src/github.com/opencontainers/runtime-tools

test_cases=(
  "create/create.t"
  "default/default.t"
  "delete/delete.t"
  "delete_only_create_resources/delete_only_create_resources.t"
  "delete_resources/delete_resources.t"
  "hooks/hooks.t"
  "hooks_stdin/hooks_stdin.t"
  "hostname/hostname.t"
  "kill/kill.t"
  "kill_no_effect/kill_no_effect.t"
  "killsig/killsig.t"
  # This case includes checking for features that are excluded from linux kernel 5.0, so even runc doesn't pass it.
  # ref. https://github.com/docker/cli/pull/2908
  # "linux_cgroups_blkio/linux_cgroups_blkio.t"
  "linux_cgroups_cpus/linux_cgroups_cpus.t"
  "linux_cgroups_devices/linux_cgroups_devices.t"
  "linux_cgroups_hugetlb/linux_cgroups_hugetlb.t"
  "linux_cgroups_memory/linux_cgroups_memory.t"
  "linux_cgroups_network/linux_cgroups_network.t"
  "linux_cgroups_pids/linux_cgroups_pids.t"
  # This case includes checking for features that are excluded from linux kernel 5.0, so even runc doesn't pass it.
  # ref. https://github.com/docker/cli/pull/2908
  # "linux_cgroups_relative_blkio/linux_cgroups_relative_blkio.t"
  "linux_cgroups_relative_cpus/linux_cgroups_relative_cpus.t" 
  "linux_cgroups_relative_devices/linux_cgroups_relative_devices.t"
  "linux_cgroups_relative_hugetlb/linux_cgroups_relative_hugetlb.t"
  "linux_cgroups_relative_memory/linux_cgroups_relative_memory.t"
  "linux_cgroups_relative_network/linux_cgroups_relative_network.t"
  "linux_cgroups_relative_pids/linux_cgroups_relative_pids.t"
  "linux_devices/linux_devices.t"
  # "linux_masked_paths/linux_masked_paths.t"
  "linux_mount_label/linux_mount_label.t"
  # This test case hangs on the Github Action. Runtime-tools has an issue filed from 2019 that the clean up step hangs. Otherwise, the test case passes.
  # Ref: https://github.com/opencontainers/runtime-tools/issues/698
  # "linux_ns_itype/linux_ns_itype.t"
  "linux_ns_nopath/linux_ns_nopath.t"
  "linux_ns_path/linux_ns_path.t"
  "linux_ns_path_type/linux_ns_path_type.t"
  # "linux_process_apparmor_profile/linux_process_apparmor_profile.t"
  "linux_readonly_paths/linux_readonly_paths.t"
  # "linux_rootfs_propagation/linux_rootfs_propagation.t"
  # "linux_seccomp/linux_seccomp.t"
  "linux_sysctl/linux_sysctl.t"
  # "linux_uid_mappings/linux_uid_mappings.t"
  "misc_props/misc_props.t"
  # "mounts/mounts.t"
  # "pidfile/pidfile.t"
  "poststart/poststart.t"
  "poststart_fail/poststart_fail.t"
  "poststop/poststop.t"
  "poststop_fail/poststop_fail.t"
  "prestart/prestart.t"
  "prestart_fail/prestart_fail.t"
  "process/process.t"
  "process_capabilities/process_capabilities.t"
  "process_capabilities_fail/process_capabilities_fail.t"
  "process_oom_score_adj/process_oom_score_adj.t"
  "process_rlimits/process_rlimits.t"
  "process_rlimits_fail/process_rlimits_fail.t"
  # "process_user/process_user.t"
  "root_readonly_true/root_readonly_true.t"
  # Record the tests that runc also fails to pass below, maybe we will fix this by origin integration test, issue: https://github.com/containers/youki/issues/56
  # "start/start.t"
  "state/state.t"
)

check_enviroment() {
  test_case=$1
  if [[ $test_case =~ .*(memory|hugetlb).t ]]; then
    if [[ ! -e "/sys/fs/cgroup/memory/memory.memsw.limit_in_bytes" ]]; then
        return 1
    fi 
  fi
}

for case in "${test_cases[@]}"; do
  if [[ ! -e "${ROOT}/integration_test/src/github.com/opencontainers/runtime-tools/validation/$case" ]]; then
    GO111MODULE=auto GOPATH=${ROOT}/integration_test make runtimetest validation-executables
    break
  fi
done


for case in "${test_cases[@]}"; do
  if ! check_enviroment $case; then
    echo "Skip $case bacause your enviroment doesn't support this test case"
    continue
  fi

  if [ $PATTERN != "." ] && [[ ! $case =~ $PATTERN ]]; then
    continue    
  fi 

  echo "Running $case"
  logfile="./log/$case.log"
  mkdir -p "$(dirname $logfile)"
  sudo RUST_BACKTRACE=1 RUNTIME=${RUNTIME} ${ROOT}/integration_test/src/github.com/opencontainers/runtime-tools/validation/$case >$logfile 2>&1
  if [ 0 -ne $(grep "not ok" $logfile | wc -l ) ]; then
      cat $logfile
      exit 1
  fi
  sleep 1
done
