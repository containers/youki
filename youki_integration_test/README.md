# Integration test

This provides a test suite to test low level OCI spec compliant container runtime

## Usage

```sh
# in root folder
$ ./build.sh
$ cd youki_integration_test
$ cp ../youki .
$ ./build.sh
# currently root access is required
$ sudo ./youki_integration_test -r ./youki
```

This provides following commandline options :

- --runtime (-r) : Required. Takes path of runtime executable to be tested. If the path is not valid, the program exits.
- --tests (-t) : Optional. Takes list of tests to be run, and runs only those tests. Format for it is : `test-grp-1::test-1,test-2 <space> test-grp-2 <space> test-grp-3::test-3 ...`. The test groups with no specific tests specified, (test-grp-2 in the example) , will run all of its tests, and in other cases, only selected tests will be run. Test groups not mentioned will be ignored.

Currently, there are following test groups and tests :

- lifecycle
  - create
  - start
  - kill
  - state
  - delete
- create
  - empty_id
  - valid_id
  - duplicate_id
