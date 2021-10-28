use crate::systemd::dbus::systemd_api::OrgFreedesktopSystemd1Manager;
use anyhow::{Context, Result};
use dbus::arg::{PropMap, RefArg, Variant};
use dbus::blocking::{Connection, Proxy};
use std::time::Duration;

/// Client is a wrapper providing higher level API and abatraction around dbus.
/// For more information see https://www.freedesktop.org/wiki/Software/systemd/dbus/
pub struct Client {
    conn: Connection,
}

impl Client {
    pub fn new() -> Result<Self> {
        let conn = Connection::new_system()?;
        Ok(Client { conn })
    }

    fn create_proxy(&self) -> Proxy<&Connection> {
        self.conn.with_proxy(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            Duration::from_millis(5000),
        )
    }

    /// start_transient_unit is a higher level API for starting a unit
    /// for a specific container under systemd.
    /// See https://www.freedesktop.org/wiki/Software/systemd/dbus for more details.
    pub fn start_transient_unit_for_container(
        &self,
        container_name: &str,
        pid: u32,
        parent: &str,
        unit_name: &str,
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
        let mut properties: Vec<(&str, Variant<Box<dyn RefArg>>)> = Vec::with_capacity(6);
        properties.push((
            "Description",
            Variant(Box::new(format!("youki container {}", container_name))),
        ));

        // if we create a slice, the parent is defined via a Wants=
        // otherwise, we use Slice=
        if unit_name.ends_with("slice") {
            properties.push(("Wants", Variant(Box::new(parent.to_owned()))));
        } else {
            log::debug!("SELECTED SCOPE");
            log::debug!("{}", parent);
            properties.push(("Slice", Variant(Box::new(parent.to_owned()))));
            properties.push(("Delegate", Variant(Box::new(true))));
        }

        properties.push(("MemoryAccounting", Variant(Box::new(true))));
        properties.push(("CPUAccounting", Variant(Box::new(true))));
        properties.push(("IOAccounting", Variant(Box::new(true))));
        properties.push(("TasksAccounting", Variant(Box::new(true))));

        properties.push(("DefaultDependencies", Variant(Box::new(false))));
        properties.push(("PIDs", Variant(Box::new(vec![pid]))));

        proxy
            .start_transient_unit(unit_name, "replace", properties, vec![])
            .context("failed to start transient unit")?;
        Ok(())
    }

    pub fn list_units(&self) -> Result<()> {
        let proxy = self.create_proxy();
        let units = proxy.list_units()?;
        for unit in units {
            log::debug!("{:?}", unit);
        }
        Ok(())
    }
}
