#!/bin/sh

# This hack script is the dummy busctl command used when running tests with cross containers.

# The issue is that we cannot run systemd or dbus inside the test container without a lot 
# of hacks. For one specific test - test_task_addition, we need to check that the task
# addition via systemd manager works. We mount the host dbus socket in the test container, so 
# dbus calls work, but for the initial authentication, we use busctl which needs dbus and systemd
# to be present and running. So instead of doing all that, we simply run the container with the 
# actual test running user's uid/gid and here we echo the only relevant line from busctl's 
# output, using id to get the uid. This is a hack, but less complex than actually setting up
# and running the systemd+dbus inside the container.

echo "OwnerUID=$(id -u)"
