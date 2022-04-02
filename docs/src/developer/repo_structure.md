# Repository Structure

This page might be the most outdated page, as the structure might change at any time! Thus make sure to update this whenever there are any change in the overall structure of the whole repo. For the same reason, this does not list in detail the structure but instead gives information of main directories.

### .github

Contains workflows and files needed by those workflows.

### crates

This is the core of youki. This contains various libraries that are developed alongside of youki, and the youki binary itself.

### docs

The directory where the source of this documentation resides. The source is also divided into two parts as the docs, developer and user. Please see [Documentation documentation](./documentation_mdbook.md) for more information.

### hack

As the name suggests, contains hack scripts for patching some issues which are currently not solvable in a straightforward way, or solving the issues for which we have no idea of why they occur.

### Scripts

Contains scripts for various purposes, such as building the youki, running integration tests etc. These might be small scripts called from many other scripts, big scripts that perform a complex task or helper scripts for the main Makefile.

### tests

This contains all the integration tests for validating youki. Note that these are integration tests for start-to-end testing of youki commands, and unit tests for individual parts are in their respective source files in crates.
