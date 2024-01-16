# Runtime test

This is the binary which runs the tests inside the container process, and checks that constraints and restrictions are upheld from inside the container. This is supposed to be rust version of [runtimetest command](https://github.com/opencontainers/runtime-tools/tree/master/cmd/runtimetest) from runtime tools.

This is primarily used from the `test_inside_container` function related tests in the integration tests.

## Conventions

The main function will call the different tests functions, one by one to check that all required guarantees hold. This might be parallelized in future, but initially the tests are run serially.

The path of config spec will always be /spec.json , and this is fixed so that no additional env or cmd arg is required, and we don't need to depend on clap or manual parsing for that.

Make sure to consider failure cases, and try not to panic from any functions. If any error occur, or if some test fails, then it should write the error to the stderr, and return. The integration test will check stderr to be empty as an indication of all tests passing, and in case stderr is not empty, it will consider some test to be failing, and show the error as the contents of stderr. Thus make sure to include enough information in stderr message from failing tests to understand what failed in which test.
There is currently no convention of explicit indication of tests passing, the passing test may write `OK` or something similar to stdout, but as of now, the stdout will be completely ignored by integration test.

## Special Notes

This package must be compiled as a statically linked binary, as otherwise the rust compile will make it dynamically link to /lib64/ld-linux-x86-64.so , which is not available inside the container, and thus making the binary not usable inside the container process.

**Note** that the dynamically linked binary does not give a `segmentation fault` or similar error when tried to run inside the container, but instead gives `no such file or directory found` or `executable not found` error, even though the executable exists in the container. This made this tricky to debug correctly when originally developing, so if you decide on chaining the compilation or configuration of this , please make absolutely sure that the changes work and do not accidentally break something.

you can use

```bash
readelf -l path/to/binary | grep "program interpreter"  # should give empty output
file path/to/binary                                     # should specify statically linked in output
```

to find out if the binary is dynamically or statically linked.

Reading the Readme of integration tests can be helpful to understand how the integration tests and the runtime tests interoperate with one another.

see

<https://stackoverflow.com/questions/31770604/how-to-generate-statically-linked-executables>
<https://superuser.com/questions/248512/why-do-i-get-command-not-found-when-the-binary-file-exists>
<https://doc.rust-lang.org/cargo/reference/config.html>

for more info
