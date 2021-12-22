# Basic Setup

This section will explain how to setup youki for use. Currently the only way to get youki is to compile it from the source. Currently youki only supports Linux systems, so this documentation assumes you are using a Linux system. For running youki on other platforms, you can use Vagrant, which is explained in the last part of this section.

### Requirements

youki is written in rust, so you will need rust toolchain installed to compile it. Also if you want to use it with a higher level container engine, such as docker, you will need to install that as well. In case you want to use one of the sub-crates of youki as a dependency, you may not need docker.

The rest of document uses docker as an example when required.

- Rust(See [here](https://www.rust-lang.org/tools/install)), edition 2021
- Docker(See [here](https://docs.docker.com/engine/install))

Apart from these basic requirements, some other libraries are also required to compile and run youki. To install them on :

#### Debian, Ubuntu and related distributions

```console
$ sudo apt-get install   \
      pkg-config         \
      libsystemd-dev     \
      libdbus-glib-1-dev \
      build-essential    \
      libelf-dev \
      libseccomp-dev
```

#### Fedora, Centos, RHEL and related distributions

```console
$ sudo dnf install   \
      pkg-config     \
      systemd-devel  \
      dbus-devel     \
      elfutils-libelf-devel \
      libseccomp-devel
```

#### Getting the source

After installing the dependencies you will need to clone the youki repository if you want to use it directly :

```console
git clone git@github.com:containers/youki.git
```

Or if you want ot use it as a dependency in a Rust project, you can specify it in your Cargo.toml :

```toml
[dependencies]
...
liboci-cli = { git = "https://github.com/containers/youki.git" }
...
```

You can specify the crate that you need as a dependency in place of `liboci-cli`

#### Installing the source

If you have cloned the source, you can build it using

```console
# go into the cloned directory
cd youki

# build
./build.sh
```

This will build the youki, and put the binary at the root level of the cloned directory.

When using it as a dependency, you can use it in your source as :

```
use liboci_cli::{...}
```

You can specify the crate that you need as a dependency in place of `liboci-cli`

#### Using Vagrant to run Youki on non-Linux Platform

you can use the vagrantfile provided with the source, to setup vagrant and run youki inside it. You can see [vagrant installation](https://www.vagrantup.com/docs/installation) for how to download and setup vagrant.

Once done, you can run vagrant commands in the cloned directory to run youki inside the VM created by vagrant :

```console
# for rootless mode, which is default
vagrant up
vagrant ssh

# or if you want to develop in rootful mode
VAGRANT_VAGRANTFILE=Vagrantfile.root vagrant up
VAGRANT_VAGRANTFILE=Vagrantfile.root vagrant ssh

# in virtual machine
cd youki
./build.sh

```
