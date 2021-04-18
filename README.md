<h1 align="center">youki</h1>
<h3 align="center">Experimental implementation of the oci-runtime in Rust</h3>

<p align="center">
<a href="LICENSE">
<img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT">
</a>
</p>

# Overview
youki is an implementation of [runtime-spec](https://github.com/opencontainers/runtime-spec) in Rust, referring to [runc](https://github.com/opencontainers/runc).
This project is in the experimental stage at this point.
I think Rust is one of the best languages to implement oci-runtime, so I'm having fun experimenting with it.

# Building
Two types of building are available: devcontainer or local.
You can choose whichever you like, but the local one will only work on Linux.

## Local
### Requires
- Rust(See [here](https://www.rust-lang.org/tools/install))
- Docker

### Building
```sh
$ git clone git@github.com:utam0k/youki.git
$ cargo build
$ RUST_BACKTRACE=full YOUKI_LOG_LEVEL=debug YOUKI_MODE=/var/lib/docker/containers/ dockerd --experimental --add-runtime="youki=$(pwd)/target/x86_64-unknown-linux-gnu/debug/youki"
```

## Devcontainer
We prepared [devcontainer](https://code.visualstudio.com/docs/remote/containers) as a development environment.
If you use devcontainer for the first time, please refer to [this page](https://code.visualstudio.com/docs/remote/containers).

The following explanation assumes that devcontainer is used.
The first time it starts up will take a while, so have a cup of coffee and wait ;)

### Requires
- VSCode
- Docker

### Bulding
This commands should be run runs in your local terminal.
```sh
$ git clone git@github.com:utam0k/youki.git
$ code youki
```
And use [devcontainer](https://code.visualstudio.com/docs/remote/containers) in your vscode.

`dockerd` is already running when you start devcontainer.
You can get more information about the startup process by referring to `.devcontainer/scripts/init.sh`.

# Usage
## youki with Docker
```
$ docker run -it --rm --runtime youki hello-world
$ docker run -it --rm --runtime youki busybox
```

## Integration test
```
$ /workspaces/youki/.devcontainer/scripts/setup_test.sh # only the first time
$ /workspaces/youki/.devcontainer/scripts/test.sh
```

## HelloWorld with youki
Do `Hello, World` using the log function of Youki.
If you want to explore youki, please use it.

Try adding the following code to the line in `src/main.rs` after initializing the logger of the main function and try to `cargo build` in your terminal.
```
log::debug!("Hello, World");
```

When you run busybox, sh will start and stop.
```
$ docker run -it --rm --runtime youki --name youki busybox
```

If you run the following command in a different terminal, you will see the `Hello, World` that you added above.
```
$ docker logs youki
```

# Features
- [x] somehow works
- [x] run with docker
- [x] namespace
- [x] capabilities
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
- [ ] rlimits
- [ ] hooks

# Contribution
This project welcomes your PR and issues.
For example, refactoring, adding features, correcting English, etc.
If you need any help, you can contact me on [Twitter](https://twitter.com/utam0k).
