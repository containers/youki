#! /bin/bash
# we don't set -eu here, as some of the binaries might be potentially be missing
# and that is fine, that means they are already removed.

rm ./youki
rm ./integration_test
rm ./runtimetest
rm ./bundle.tar.gz
rm ./test.log
exit 0 # unconditionally return zero