use crate::systemd::dbus::systemd_api::OrgFreedesktopSystemd1Manager;
use anyhow::Result;
use dbus::arg::{PropMap, Variant};
use dbus::blocking::Connection;
use std::time::Duration;

/// Client is a wrapper providing higher level API and abatraction around dbus.
/// For more information see https://www.freedesktop.org/wiki/Software/systemd/dbus/
pub struct Client {
    conn: Connection,
}

impl Client {
    pub fn new() -> Result<Self> {
        let conn = Connection::new_session()?;
        Ok(Client { conn })
    }

    /// start_transient_unit is a higher level API for starting a unit
    /// for a specific container under systemd.
    /// See https://www.freedesktop.org/wiki/Software/systemd/dbus for more details.
    pub fn start_transient_unit_for_container(
        &self,
        container_name: &str,
        unit_name: &str,
        mut properties_map: PropMap,
    ) -> Result<()> {
        // To view and introspect the methods under the 'org.freedesktop.systemd1' destination
        // and object path under it use the following command:
        // `gdbus introspect --system --dest org.freedesktop.systemd1 --object-path /org/freedesktop/systemd1`
        let proxy = self.conn.with_proxy(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            Duration::from_millis(5000),
        );

        // To align with runc, yuoki will always add the following properties to its container units:
        // - CPUAccounting=true
        // - IOAccounting=true (BlockIOAccounting for cgroup v1)
        // - MemoryAccounting=true
        // - TasksAccounting=true
        // see https://github.com/opencontainers/runc/blob/6023d635d725a74c6eaa11ab7f3c870c073badd2/docs/systemd.md#systemd-cgroup-driver
        // for more details.
        properties_map.insert(
            "Description".to_string(),
            Variant(Box::new(format!("youki container {}", container_name))),
        );

        let slice = "machine.slice";
        // if we create a slice, the parent is defined via a Wants=
        // otherwise, we use Slice=
        if unit_name.ends_with("slice") {
            properties_map.insert("Wants".to_string(), Variant(Box::new(slice.to_owned())));
        } else {
            properties_map.insert("Slice".to_string(), Variant(Box::new(slice.to_owned())));
        }

        properties_map.insert("MemoryAccounting".to_string(), Variant(Box::new(true)));
        properties_map.insert("CPUAccounting".to_string(), Variant(Box::new(true)));
        properties_map.insert("IOAccounting".to_string(), Variant(Box::new(true)));
        properties_map.insert("TasksAccounting".to_string(), Variant(Box::new(true)));

        let mut properties = vec![];
        for (name, variant) in &mut properties_map.iter() {
            let s: &str = &*name;
            properties.push((s, variant.to_owned()));
        }
        //proxy.start_transient_unit(unit_name, "replace", properties, vec![])?;
        Ok(())
    }
}
