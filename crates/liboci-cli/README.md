# liboci-cli

This is a crate to parse command line arguments for OCI container
runtimes as specified in the [OCI Runtime Command Line
Interface](https://github.com/opencontainers/runtime-tools/blob/master/docs/command-line-interface.md).

## Implemented subcommands

| Command    | liboci-cli | CLI Specification | runc | crun | youki |
| :--------: | :--------: | :---------------: | :--: | :--: | :---: |
| create     | ✅         | ✅                | ✅   | ✅   | ✅    |
| start      | ✅         | ✅                | ✅   | ✅   | ✅    |
| state      | ✅         | ✅                | ✅   | ✅   | ✅    |
| kill       | ✅         | ✅                | ✅   | ✅   | ✅    |
| delete     | ✅         | ✅                | ✅   | ✅   | ✅    |
| checkpoint |            |                   | ✅   | ✅   |       |
| events     | ✅         |                   | ✅   |      | ✅    |
| exec       | ✅         |                   | ✅   | ✅   | ✅    |
| features   | ✅         |                   | 🏴   |     |      |
| list       | ✅         |                   | ✅   | ✅   | ✅    |
| pause      | ✅         |                   | ✅   | ✅   | ✅    |
| ps         | ✅         |                   | ✅   | ✅   | ✅    |
| restore    |            |                   | ✅   | ✅   |       |
| resume     | ✅         |                   | ✅   | ✅   | ✅    |
| run        | ✅         |                   | ✅   | ✅   | ✅    |
| spec       | ✅         |                   | ✅   | ✅   | ✅    |
| update     |            |                   | ✅   | ✅   |       |

🏴 The `features` subcommand is [implemented](https://github.com/opencontainers/runc/pull/3296)
in `runc`, but not documented.
