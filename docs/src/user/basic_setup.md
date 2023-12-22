# Basic Setup

This explains the requirements for compiling Youki as a binary, to use it as a low-level container runtime, or to depend once of its crates as dependency for your own project.

Youki currently only supports Linux Platform, and to use it on other platform you will need to use some kind of virtualization. The repo itself provides Vagrantfile that provides basic setup to use Youki on non-Linux system using Vagrant. The last sub-section explains using this vagrantfile.

Also note that Youki currently only supports and expects systemd as init system, and would not work on other systems. There is currently work on-going to put systemd dependent features behind a feature flag, but till then you will need a systemd enabled system to work with Youki.

## Requirements

As Youki is written in Rust, you will need to install and setup Rust toolchain to compile it. The instructions for that can be found on Rust's official site [here](https://www.rust-lang.org/tools/install).

You can use Youki by itself to start and run containers, but it can be a little tedious, as it is a low-level container runtime. You can use a High-level container runtime, with its runtime set to Youki, so that it will be easier to use. Both of these are explained in the [Basic Usage](./basic_usage.md). For using it along with an high-level runtime, you will to install one such as Docker or Podman. This documentation uses Docker in its examples, which can be installed from [here](https://docs.docker.com/engine/install).

To compile and run, Youki itself depends on some underlying libraries being installed. You can install them using your respective package manager as shown below.

### Debian, Ubuntu and related distributions

```console
$ sudo apt-get install    \
      pkg-config          \
      libsystemd-dev      \
      build-essential     \
      libelf-dev          \
      libseccomp-dev      \
      libclang-dev        \
      libssl-dev
```

### Fedora, CentOS, RHEL and related distributions

```console
$ sudo dnf install          \
      pkg-config            \
      systemd-devel         \
      elfutils-libelf-devel \
      libseccomp-devel      \
      clang-devel           \
      openssl-devel
```

---

## Quick install

Install from the GitHub release.
Note that this way also requires the aforementioned installation.

<!--youki release begin-->
```console
$ wget -qO youki-0.3.1.tar.gz https://github.com/containers/youki/releases/download/v0.3.1/youki-0.3.1-$(uname -m).tar.gz
$ tar -zxvf youki-0.3.1.tar.gz youki
# Maybe you need root privileges.
$ mv youki /usr/local/bin/youki
$ rm youki-0.3.1.tar.gz
```
<!--youki release end-->

## Getting the source

Currently Youki can only be installed from the source code itself, so you will need to clone the Youki GitHub repository to get the source code for using it as a runtime. If you are using any crates of Youki as dependency you need to do this step, as Cargo will automatically clone the repository for you.

To clone the repository, run

```console
$ git clone https://github.com/containers/youki.git
```

This will create a directory named youki in the directory you ran the command in. This youki directory will be referred to as root directory throughout the documentation.

## Installing the source

Once you have cloned the source, you can build it with [just](https://github.com/casey/just#installation) :

```console
# go into the cloned directory
$ cd youki
$ just youki-dev # or youki-release
$ ./youki -h # get information about youki command
```

This will build the Youki binary, and put it at the root level of the cloned directory, that is in the youki/ .

---

## Using sub-crates as dependency

To use any of the sub-crate as a dependency in your own project, you can specify the dependency as follows,

```toml
[dependencies]
...
liboci-cli = { git = "https://github.com/containers/Youki.git" }
...
```

Here we use `liboci-cli` as an example, which can be replaced by the sub-crate that you need.

Then you can use it in your source as

```
use liboci_cli::{...}
```

---

## Using Vagrant to run Youki on non-Linux Platform

As explained before, Youki only support Linux, and to build/use it on non-Linux Platforms, you will need to use some kind of virtualization. The repo provides a Vagrantfile to do the required VM setup using Vagrant, which can be installed from [here](https://www.vagrantup.com/docs/installation).

Once installed and setup, you can run vagrant commands in the cloned directory to run Youki inside the VM created by vagrant :

```console
# in the youki directory

# for rootless mode, which is default
$ vagrant up
$ vagrant ssh

# or if you want to develop in rootful mode
$ VAGRANT_VAGRANTFILE=Vagrantfile.root vagrant up
$ VAGRANT_VAGRANTFILE=Vagrantfile.root vagrant ssh

# in virtual machine
$ cd youki
$ just youki-dev # or youki-release
```
