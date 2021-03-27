<h1 align="center">youki</h1>
<h3 align="center">Rust experimental implementation of the oci-runtime</h3>

<p align="center">
<a href="LICENSE">
<img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT">
</a>
</p>

## Overview
youki is an implementation of [runtime-spec](https://github.com/opencontainers/runtime-spec) in Rust, referring to [runc](https://github.com/opencontainers/runc).
This project is in the experimental stage at this point.
I think Rust is one of the best languages to implement oci-runtime, so I'm having fun experimenting with it.

## Try and play
We prepared [devcontainer](https://code.visualstudio.com/docs/remote/containers) as a development environment.
The following explanation assumes that devcontainer is used.
At this stage, it sometimes fails to start the container, but don't worry about it, just retry.
The first time it starts up will take a while, so have a cup of coffee and wait ;)

### Requires
- vscode
- docker

### youki with Docker
Run the following command in a terminal inside devcontainer.
`dockerd` is already running when you start devcontainer.
See `.devcontainer/scripts/init.sh` for details.
```
$ docker run -it --rm --runtime youki hello-world
$ docker run -it --rm --runtime youki busybox
```

### Integration test
```
$ /workspaces/youki/.devcontainer/scripts/setup_test.sh # only the first time
$ /workspaces/youki/.devcontainer/scripts/test.sh
```

### HelloWorld with youki
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


## Features
- [x] somehow works
- [x] run with docker
- [x] namespace
- [ ] rlimit
- [ ] cgroup
- [ ] hook

## Contribution
This project welcomes your PR and issues.
For example, refactoring, adding features, correcting English, etc.
If you need any help, you can contact me on [Twitter](https://twitter.com/utam0k).