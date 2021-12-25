# libseccomp

This crate provides Rust FFI bindings to [libseccomp](https://github.com/seccomp/libseccomp). This is adapted from code generated using rust-bindgen from a C header file. This also manually fixes some of the issues that occur as rust-bindgen has some issues when dealing with C function macros.
