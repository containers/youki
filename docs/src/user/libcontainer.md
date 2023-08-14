# libcontainer

This crate provides functionality for creating and managing containers. Youki itself uses this crate to manage and control the containers.

This exposes several modules, each dealing with a specific aspect of working with containers.

- `apparmor` : functions that deal with apparmor, which is a Linux Kernel security module to control program capabilities with per program profiles.

- `capabilities` : this has functions related to setting and resetting specific capabilities, as well as to drop extra privileges from container process.

- `config` : this exposes `YoukiConfig` struct, which contains a subset of the data in the `config.json`. This is the subset that is needed when starting or managing containers after creation, and rather than parsing and passing around whole `config.json`, the smaller `YoukiConfig` is passed, which is comparatively faster.

- `container` : This is the core of the container module, and contains sub-modules and structs that deal with the container lifecycle including creating, starting, stopping and deleting containers.

- `hooks` : exposes function `run_hooks`, which is used to run various container lifecycle hooks as specified in oci-spec.

- `namespaces` : exposes `Namespaces` struct, which deals with applying namespaces to a container process.

- `notify_socket` : this contains `NotifyListener` struct, which is used internally to communicate between the main youki process and the forked container processes.

- `process` : a module which exposes functions related to forking the process, setting up the namespaces and starting the container process with correct namespaces.

- `rootfs` : this contains modules which deal with rootfs, which is minimal filesystem that is provided to the container.

- `user_ns` : this deals with running containers in with new user namespace, usually rootless containers will use this, that is running containers without needing root permissions.

- `seccomp` : this deals with setting up seccomp for container process. It uses libseccomp crate in order to do that.

- `signal` : this provides simple wrappers for unix signal, so that parsing them from their names or signal numbers is easier.

- `syscall` : this provides a trait `Syscall`, which is used to abstract over several functionalities which need to call libc functions. This allows the other parts of library to use those functions without having to deal with implementation details.

- `tty` : this deals with setting up the tty for the container process.

- `utils` : provides various utility functions, such as `parse_env` to parse the env variables, `get_cgroups_path`, `create_dir_all_with_mode` etc.
