# Dbus Native

This module is the native implementation of dbus connection functionality used for connecting with systemd via dbus. Refer to [this issue discussion](https://github.com/containers/youki/issues/2208) following for the discussion regarding moving away from existing dbus-interfacing library.

Note that this implements the minimal required functionality for youki to use dbus, and thus does not have all the dbus features.

- Refer to see [dbus specification](https://dbus.freedesktop.org/doc/dbus-specification.html) and [header format](https://dbus.freedesktop.org/doc/api/html/structDBusHeader.html) for the individual specifications.

- For systemd interface and types, you can generate the following file and take help from the auto-generated functions
`dbus-codegen-rust -s -g -m None -d org.freedesktop.systemd1 -p /org/freedesktop/systemd1`, see https://github.com/diwic/dbus-rs