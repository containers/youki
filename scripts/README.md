# Scripts

This stores various scripts that do various things. These can be intended to be used directly, or can be for using from some other scripts or Makefiles.

#### Note

Please use `set -e` in start of every script. This will make the operation fail even if any single command fails in that script. Without it, the script will continue after the failing command, and might create knock-on effect of incorrect results. In case you expect some step to fail, then handle the failure of that directly than checking some condition to see if command is successful in rest of the script.
