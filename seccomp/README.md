# Bindings to libseccomp

This crate contains a rust FFI binding to
[libseccomp](https://github.com/seccomp/libseccomp).

The code is adapted from auto generated code using
[rust-bindgen](https://github.com/rust-lang/rust-bindgen). The `rust-bindgen`
has some issue with detecting function macro, which `libseccomp` uses. We
decided to manually fix the issue and include the bindings in this crate.

The header file used: <https://github.com/seccomp/libseccomp/blob/main/include/seccomp.h.in>
