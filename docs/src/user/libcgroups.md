# libcgroups

`libcgroups` provide a Rust interface over cgroups v1 and v2.

It exposes the modules :

- common
- stats
- systemd
- test_manager
- v1
- v2

### common

This module contains common features for cgroups v1 and v2, such as

- trait CgroupManager, which gives public interface to

  - add a task to a cgroup
  - apply resource restriction
  - remove a cgroup
  - freezer cgroup state control
  - get stats from a cgroup
  - get pids belonging to the cgroup

- functions `write_cgroup_file_str` and `write_cgroup_file` which writes data to a cgroup file
- function `read_cgroup_file` which reads data from given cgroup file
- function `get_cgroup_setup` which returns setup of cgroups (v1,v2, hybrid) on the system

### stats

This module contains structs and functions to work with statistics for a cgroup, such as

- struct Stats which contains all following individual stat structs
  - CpuStats : stores usage and throttling information
  - MemoryStats : stores usage of memory, swap and memory combined, kernel memory, kernel tcp memory and other memory stats
  - PidStats : contains current number of active pids and allowed number of pids
  - BlkioStats : contains block io related stats such as number of bytes transferred from/to by a device in cgroup, number of io operations done by a device in cgroup, device access and queue information
  - HugeTlbStats : stats for huge TLB , containing usage, max_usage and fail count
- function `supported_page_size` which returns hugepage size supported by the system
- functions to operate with data in cgroups files such as
  - `parse_single_value` : reads file expecting it to have a single value, and returns the value
  - `parse_flat_keyed_data` : parses cgroup file data which is in flat keyed format (key value)
  - `parse_nested_keyed_data` : parses cgroup file data which is in nested keyed format (key list of values)
  - `parse_device_number` : parses major and minor number of device

### systemd

This module contains functions and modules to deal with systemd, as currently youki depends on systemd. This exposes

- function `booted` to check if the system was booted with systemd or not
- module controller_type, which contains
  - enum ControllerType which is used to specify controller types
- module manager, which contains
  - struct Manager, which is the cgroup manager, and contain information about the root cgroups path, path for the specific cgroups, client to communicate with systemd etc. This also implements CgroupManager trait.

### test_manager

This exposes a TestManager struct which can be used as dummy for testing purposes, which also implements CgroupManager.

### v1 and v2

These modules contains modules and fucntions related to working with cgroups v1 and v2. They expose respective cgroups version specific mangers, and some utility functions to get mount points (for v1 and v2), get subsystem mount points (for v1) and get available controllers (for v2) etc.

For cgroups v2, it also exposes devices module, which gives functions for working with bpf such as load a bpf program, query info of a bpf program, attach and detach a bpf program to a cgroup, etc.
