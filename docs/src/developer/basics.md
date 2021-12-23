# Basics

This section has the basic information and resources needed to work with any part of youki. This also assumes that you already know Rust. If you don't yet, you should probably learn it first before contributing here. Good resources for that can be found on the Rust's [official site](https://www.rust-lang.org/learn).

Youki is a low level container runtime, which aims to deal with creation and management of Linux containers on a low level. Some of other such low-level runtimes are [runc](https://github.com/opencontainers/runc) and [crun](https://github.com/containers/crun). These are usually used by a higher-level runtime to actually create and manage containers, while the higher level runtime provides a much easier interface for users.

Before you start working on developing youki, you should go through the User documentation as it specifies the requirements and setup for running youki. For developing youki, you will need to install the dependencies and clone the repo, as specified in the [Basic Setup](../user/basic_setup.md) and [Basic Usage](../user/basic_usage.md) sections.

## Resources

The youki is an OCI-spec compliant runtime. OCI or Open Container Initiative provides a common specification to be used by runtimes, so that there would be an uniform interface, and users or other programs need not deal with each runtime's interface separately. Their main GitHub page is at [https://github.com/opencontainers](https://github.com/opencontainers), and information about the specifications can be found there.

The specification of the runtime itself can be found at [https://github.com/opencontainers/runtime-spec/blob/master/runtime.md](https://github.com/opencontainers/runtime-spec/blob/master/runtime.md).

Another good resource is man pages, one main place where they can be found is at [https://man7.org/](https://man7.org/). These have very good explanation of the programming interfaces for Linux libraries and various kernel features. To find information about a certain function or feature, you can search `man <feature-name>` in a search engine, or you can search at the [site itself](https://man7.org/linux/man-pages/index.html). These come in handy when dealing with low level system interfaces.

Happy developing!!!
