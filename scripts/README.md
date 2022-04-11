# Scripts

This stores various scripts that do various things. These can be intended to be used directly, or can be for using from some other scripts or Makefiles.

#### Note

Please use `set -e` at the start of every script. This will ensure that the operation fails if any single command fails in that script. Without it, the script will continue after the failing command and might create a knock-on effect of incorrect results. In case you expect some step to fail, handle the failure directly rather than checking some condition to see if the command is successful in the rest of the script.
