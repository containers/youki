# youki: A container runtime in Rust

![youki logo](docs/youki.png)

youki is an implementation of [runtime-spec](https://github.com/opencontainers/runtime-spec) in Rust, referring to [runc](https://github.com/opencontainers/runc).

# Motivation
Here is why we are writing a new container runtime in Rust.
- Rust is one of the best languages to implement oci-runtime. Many container tools are written in Go. It's all very nice products. However, the container runtime requires the use of system calls, which requires a bit of special handling when implemented in Go. This is too tricky(e.g. *namespaces(7)*, *fork(2)*); with Rust, it's not that tricky and you can use system calls. Also, unlike C, Rust provides the benefit of memory management.
- The development of [railcar](https://github.com/oracle/railcar) has been suspended. This project was very nice but is no longer being developed. This project is inspired by it.
- We have fun implementing this. In fact, this may be the most important.

# Status of youki
youki is not at the practical stage yet. However, it is getting closer to practical use, running with docker and passing all the default tests provided by [opencontainers/runtime-rools](https://github.com/opencontainers/runtime-tools).
![youki demo](docs/demo.gif)

# Getting Started
Local build is only supported on linux.
For other platforms, please use the devcontainer that we prepared.
## Requires
- Rust(See [here](https://www.rust-lang.org/tools/install))
- Docker

## Building
```sh
$ git clone git@github.com:utam0k/youki.git
$ cd youki
$ cargo build
```

## Usage
Starting the docker daemon.
```
$ dockerd --experimental --add-runtime="youki=$(pwd)/target/x86_64-unknown-linux-gnu/debug/youki"
```

You can use youki in a different terminal to start the container.
```
$ docker run -it --rm --runtime youki busybox
```

### Integration test
You can use [opencontainers/runtime-rools](https://github.com/opencontainers/runtime-tools) to do integration testing used in CI.
See it's documentation for details.

# Design and implementation of youki
TBD

# Features
- [x] run with docker
- [x] pivot root
- [x] mount devices
- [x] namespace
- [x] capabilities
- [x] rlimits
- [ ] cgroups v1
    - [x] devices
    - [ ] cpu
    - [ ] cpuacct
    - [ ] cpuset
    - [ ] memory
    - [ ] freezer
    - [ ] net_cls
    - [ ] blkio
    - [ ] perf_event
    - [ ] net_prio
    - [ ] hugetlb
    - [ ] pids
    - [ ] rdma
- [ ] hooks

# Contribution
This project welcomes your PR and issues.
For example, refactoring, adding features, correcting English, etc.
If you need any help, you can contact me on [Twitter](https://twitter.com/utam0k).
