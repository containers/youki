# Runtime Test

This crate provides a binary which is used by integration tests to verify that the restrictions and constraints applied to the container are upheld by the container process, from inside the container process. This runs the tests one-by-one, and the failing test prints the error to the stderr.

## Notes

This binary must be compiled with the option of static linking to crt0 given to the rustc. If compiled without it, it will add a linking to /lib64/ld-linux-x86-64.so . The binary compiled this way cannot be run inside the container process, as they do not have access to /lib64/... Thus the runtime test must be statically linked to crt0.

Also this option can be given through .cargo/config.toml rustflags option, but this works only if the cargo build is invoked within the runtimetest directory. If invoked from the project root, the .cargo/config in the project root will take preference and the rustflags will be ignored.

To see if a binary can be run inside the container process, run

```console
readelf -l path/to/binary |grep "program interpreter"
```

`[Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]` means that the binary is not statically linked, and cannot be run inside the container process. If the above command gives no output, that means it does not require any program interpreter and can be run inside the container.

Another way is to run

```console
file path/to/binary
```

```console
./youki: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, BuildID[sha1]=...., for GNU/Linux 3.2.0, with debug_info, not stripped`
```

This output indicates that the binary is dynamically linked, thus cannot be run inside the container process

```console
./runtimetest: ELF 64-bit LSB executable, x86-64, version 1 (GNU/Linux), statically linked, BuildID[sha1]=...., for GNU/Linux 3.2.0, with debug_info, not stripped
```

This output indicates that the binary is statically linked, and can be run inside the container process

Some links to help :

- [how to generate static executable](https://stackoverflow.com/questions/31770604/how-to-generate-statically-linked-executables)
- [understanding the error which dynamically linked library gives](https://superuser.com/questions/248512/why-do-i-get-command-not-found-when-the-binary-file-exists)
- [Rust cargo config for rustflags](https://doc.rust-lang.org/cargo/reference/config.html)
