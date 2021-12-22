# libcontainer

This is a library that provides utilities to setup and run containers. Youki itself uses this crate to manage and control the containers.

This exposes several modules, each dealing with a specific aspect of working with containers.

- apparmor : functions that deal with apparmor, which is a Linux Kernel security module to control program capabilities with per program profiles.

- capabilities : this has functions related to set and reset specific capabilities, as well as to drop extra privileges.

- config : this exposes YoukiConfig struct, which contains a subset of the data in the config.json. This subset is needed when starting or managing containers after creation, and rather than parsing and passing around whole config.json, this smaller YoukiConfig is passed, which is comparatively faster.

- container : This is the core of the container module, and contains modules and structs that deal with the container lifecycle including creating, starting , stopping and deleting containers.

- hooks : exposes function run_hooks, which is used to run various container lifecycle hooks as specified in oci-spec

- namespaces : exposes Namespaces struct, which deals with applying namespaces to a container process

- notify_socket : this has NotifyListener struct, which is used internally to communicate between the main youki process and the forked container process

- process : a module which exposes functions related to forking the process, starting the container process with correct namespaces and setting up the namespaces

- rootfs : this contains modules which deal with rootfs, which is minimal filesystem

- rootless : this deals with running containers rootless, that is without needing root privileges on the host system

- seccomp : this deals with setting up seccomp for container process, this uses libseccomp.

- signal : this provide simple wrappers for unix signal's ascii names/numbers.

- syscall : this provides a trail Syscall, which is used to abstract over several functions which need to call libc functions, so that other parts of library can use them without having to deal with implementation details.

- tty : this deals with setting up the tty for the container process

- utils : provides various utility functions such as parse_env to parse the env variables, do_exec to do an exec syscall and execute a binary, get_cgroups_path, create_dir_all_with_mode etc.
