_This is a draft for a high level documentation of Youki. After finished this is intended to provide how control flow and high level functioning of Youki happens for development purposes._

## Some reference links

These are references to various documentations and specifications, which can be useful to understand commands and constraints.

- [OCI runtime specification] : The specification for a container runtime. Any OCI complaisant runtime must follow this.
- [runc man pages] : has information on various commandline options supported by runc, can be used to understand commands and their options.
- [cgroups man page](https://man7.org/linux/man-pages/man7/cgroups.7.html) : contains information about cgroups, their creation, deletion etc.
- [pseudoterminal man page](https://man7.org/linux/man-pages/man7/pty.7.html) : Information about the pseudoterminal system, useful to understand console_socket parameter in create subcommand
- [Unix Sockets man page](https://man7.org/linux/man-pages/man7/unix.7.html) : Useful to understand sockets
- [prctl man page](https://man7.org/linux/man-pages/man2/prctl.2.html) : Process control man pages
- [OCI Linux spec](https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md) : Linux specific section of OCI Spec
- [pipe2 man page](https://man7.org/linux/man-pages/man2/pipe.2.html) : definition and usage of pipe2

---

## Control flow diagram

This is diagram as given in #14, which is not actually how this works, but helpful to understand overall flow. Someone needs to check and correct.

```mermaid
sequenceDiagram
participant U as User
participant D as Docker
participant YP as Youki(Parent Process)
participant YI as Youki(Init Process)


U ->> D : $ docker run --rm -it --runtime youki $image
D ->> YP : youki create $container_id
YP ->> YI : clone(2) to create new process and new namespaces
YI ->> YP : set user id mapping if entering into usernamespaces
YI ->> YI : configure resource limits, mount the devices, and etc.
YI -->> YP : ready message (Unix domain socket)
YP ->> : set cgroup configuration for YI
YP ->> D : exit $code
D ->> YP : $ youki start $container_id
YP -->> YI : start message (Unix domain socket)
YI ->> YI : run the commands in dockerfile
D ->> D : monitor pid written in pid file
D ->> U : exit $code

```

---

## Control flow

### main invocation

On invoking Youki, main function parses args passed to it, which contains directory path to store container state (check runc . 8 . md in [runc man pages]), optional log path and log format string and a subcommand such as create, delete etc.

From there it matches subcommand arg with possible subcommand and takes appropriate actions, such as creating a new container, deleting a container erc.

### create container

One thing to note is that in the end, container is just another process in Linux. It has specific/different control group, namespace, using which program executing in it can be given impression that is is running on a complete system, but on the system which it is running, it is just another process, and has attributes such as pid, file descriptors, etc. associated with it like any other process.

When given create command, Youki will load the specification, configuration, sockets etc., use clone syscall to create the container process (init process),applies the limits, namespaces, and etc. to the cloned container process. The container process will wait on a unix domain socket before exec into the command/program.

The main youki process will setup pipes used to communicate and syncronize with the init process. The init process will notify the youki process that it is ready and start to wait on a unix domain socket. The youki process will then write the container state and exit.

- [mio Token definition](https://docs.rs/mio/0.7.11/mio/struct.Token.html)
- [oom-score-adj](https://dev.to/rrampage/surviving-the-linux-oom-killer-2ki9)
- [unshare man page](https://man7.org/linux/man-pages/man1/unshare.1.html)
- [user-namespace man page](https://man7.org/linux/man-pages/man7/user_namespaces.7.html)
- [wait man page](https://man7.org/linux/man-pages/man3/wait.3p.html)

### Process

This handles creation of the container process. The main youki process creates the container process (init process) using clone syscall. The main youki process will set up pipes used as message passing and synchronization mechanism with the init process. Youki uses clone instead of fork to create the container process. Using clone, Youki can directly pass the namespace creation flag to the syscall. Otherwise, if using fork, Youki would need to fork two processes, the first to enter into usernamespace, and a second time to enter into pid namespace correctly.

- [clone(2) man page](https://man7.org/linux/man-pages/man2/clone.2.html)

### Container

This contains structure represent and functions related to container process and its state and status.

### Command

This contains a trait to wrap commonly required syscalls, so that they can be abstracted from implementation details for rest of Youki.
This also provides implementation for Linux syscalls for the trait.

- [pivot_root man page](https://man7.org/linux/man-pages/man2/pivot_root.2.html)
- [umount2 man page](https://man7.org/linux/man-pages/man2/umount2.2.html)
- [capabilities man page](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [unshare man page](https://man7.org/linux/man-pages/man2/unshare.2.html)

## Capabilities

This has functions related to set and reset specific capabilities, as well as to drop extra privileges

- [Simple explanation of capabilities](https://blog.container-solutions.com/linux-capabilities-in-practice)
- [man page for capabilities](https://man7.org/linux/man-pages/man7/capabilities.7.html)

## Info

This is primarily for printing info about system running youki, such as OS release, architecture, cpu info, cgroups info etc. , as this info can be helpful when reporting issues.

- [about /etc/os-release](https://www.freedesktop.org/software/systemd/man/os-release.html)

## Namespaces

This has functions related to setting of namespaces to the calling process

- [CLONE_NEWUSER flag](https://man7.org/linux/man-pages/man2/clone.2.html)

[oci runtime specification]: https://github.com/opencontainers/runtime-spec/blob/master/runtime.md
[runc man pages]: (https://github.com/opencontainers/runc/blob/master/man/runc.8.md)
