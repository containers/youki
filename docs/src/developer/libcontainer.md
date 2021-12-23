# libcontainer

This is one of the core crates part of the youki workspace. This deals with the actual creation and management of the container processes, and provides functions and structs for the same.

Remember, that in the end, a container is just another process in Linux, which has control groups, namespaces, pivot_root and other mechanisms applied to it. The program executing has the impression that is is running on a complete system, but from the host system's perspective, it is just another process, and has attributes such as pid, file descriptors, etc. associated with it like any other process.

When given the create command, Youki will load the specification, configuration, sockets etc., use clone syscall to create the container process (init process), applies the limits, namespaces, and etc. to the cloned container process. The container process will wait on a unix domain socket before executing the command/program.

The main youki process will setup pipes to communicate and synchronize with the intermediate and init process. The init process will notify the intermediate process, and then intermediate process to the main youki process that it is ready and start to wait on a unix domain socket. The youki process will then write the container state and exit.

The following are some resources that can help understand with various Linux features used in the code of this crate.

- [mio Token definition](https://docs.rs/mio/0.7.11/mio/struct.Token.html)
- [oom-score-adj](https://dev.to/rrampage/surviving-the-linux-oom-killer-2ki9)
- [unshare man page](https://man7.org/linux/man-pages/man1/unshare.1.html)
- [user-namespace man page](https://man7.org/linux/man-pages/man7/user_namespaces.7.html)
- [wait man page](https://man7.org/linux/man-pages/man3/wait.3p.html)

The main youki process creates the intermediate process and the intermediate process creates the container process (init process). The hierarchy is: `main youki process -> intermediate process -> init process`

The main youki process will set up pipes used as message passing and synchronization mechanism with the init process. The reason youki needs to create/fork two process instead of one is due to the user and pid namespaces. In rootless container, we need to first enter user namespace, since all other namespaces requires CAP_SYSADMIN. When unshare or set_ns into pid namespace, only the children of the current process will enter into a different pid namespace. As a result, we must first fork a process to enter into user namespace, call unshare or set_ns for pid namespace, then fork again to enter into the correct pid namespace.

Note: clone(2) offers us the ability to enter into user and pid namespace by creatng only one process. However, clone(2) can only create new pid namespace, but cannot enter into existing pid namespaces. Therefore, to enter into existing pid namespaces, we would need to fork twice. Currently, there is no getting around this limitation.

- [fork(2) man page](https://man7.org/linux/man-pages/man2/fork.2.html)
- [clone(2) man page](https://man7.org/linux/man-pages/man2/clone.2.html)
- [pid namespace man page](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html)

This has functions related to set and reset specific capabilities, as well as to drop extra privileges

- [Simple explanation of capabilities](https://blog.container-solutions.com/linux-capabilities-in-practice)
- [man page for capabilities](https://man7.org/linux/man-pages/man7/capabilities.7.html)

This has functions related to setting of namespaces to the calling process

- [CLONE_NEWUSER flag](https://man7.org/linux/man-pages/man2/clone.2.html)
