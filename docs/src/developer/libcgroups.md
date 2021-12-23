# libcgroups

This crate provides and interface for working with cgroups in Linux. cgroups or control groups are Linux kernel feature which can be used to fine-control resources and permissions given to a particular process or a group of processes. You can read more about them on the [cgroups man page](https://man7.org/linux/man-pages/man7/cgroups.7.html).

The initial version of cgroups is called the version 1 was implemented in kernel 2.6.24 , later in kernel version 4.5, a new version of cgroups was released, aimed to solve issues with v1, and was the version v2.

This crate exposes modules to work with both of the version, as well as structs needed to store cgroups data. Apart from these two modules, cgroups crate also exposes utility functions to read cgroups files and parse their data, and a stats module that exposes the structs and functions to parse and store the statistics related to cgroups, such as cpu and memory usage, Huge TLB size and hits etc.

As youki currently depends on systemd as an init system, this crate also exposes module systemd, which provides interface for working with systemd related operations. [systemd resource control](https://www.freedesktop.org/software/systemd/man/systemd.resource-control.html) is a good place to read more about systemd and its involvement in resource control.
