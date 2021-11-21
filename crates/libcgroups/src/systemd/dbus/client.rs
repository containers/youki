use crate::systemd::dbus::systemd_api::OrgFreedesktopSystemd1Manager;
use anyhow::{Context, Result};
use dbus::arg::{RefArg, Variant};
use dbus::blocking::{Connection, Proxy};
use std::collections::HashMap;
use std::time::Duration;

/// Client is a wrapper providing higher level API and abatraction around dbus.
/// For more information see https://www.freedesktop.org/wiki/Software/systemd/dbus/
pub struct Client {
    conn: Connection,
}

impl Client {
    /// Uses the system bus to communicate with systemd
    pub fn new_system() -> Result<Self> {
        let conn = Connection::new_system()?;
        Ok(Client { conn })
    }

    /// Uses the session bus to communicate with systemd
    pub fn new_session() -> Result<Self> {
        let conn = Connection::new_session()?;
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
    pub fn start_transient_unit(
        &self,
        container_name: &str,
        pid: u32,
        parent: &str,
        unit_name: &str,
    ) -> Result<()> {
        // To view and introspect the methods under the 'org.freedesktop.systemd1' destination
        // and object path under it use the following command:
        // `gdbus introspect --system --dest org.freedesktop.systemd1 --object-path /org/freedesktop/systemd1`
        let proxy = self.create_proxy();

        // To align with runc, youki will always add the following properties to its container units:
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
            .with_context(|| {
                format!(
                    "failed to start transient unit {}, parent is {}",
                    unit_name, parent
                )
            })?;
        Ok(())
    }

    pub fn stop_transient_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.create_proxy();

        proxy
            .stop_unit(unit_name, "replace")
            .with_context(|| format!("failed to stop unit {}", unit_name))?;
        Ok(())
    }

    pub fn set_unit_properties(
        &self,
        unit_name: &str,
        properties: &HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<()> {
        let proxy = self.create_proxy();

        let props = properties
            .iter()
            .map(|p| (*p.0, Variant(p.1.box_clone())))
            .collect();

        proxy
            .set_unit_properties(unit_name, true, props)
            .with_context(|| format!("failed to set properties for unit {:?}", unit_name))?;
        Ok(())
    }

    pub fn systemd_version(&self) -> Result<u32> {
        let proxy = self.create_proxy();

        let version = proxy
            .version()
            .context("dbus request failed")?
            .chars()
            .skip_while(|c| c.is_alphabetic())
            .take_while(|c| c.is_numeric())
            .collect::<String>()
            .parse::<u32>()
            .context("could not parse systemd version")?;

        Ok(version)
    }
}
