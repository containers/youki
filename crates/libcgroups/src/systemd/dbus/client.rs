use crate::systemd::dbus::systemd_api::OrgFreedesktopSystemd1Manager;
use dbus::arg::{RefArg, Variant};
use dbus::blocking::{Connection, Proxy};
use std::collections::HashMap;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum SystemdClientError {
    #[error("dbus error: {0}")]
    DBus(#[from] dbus::Error),
    #[error("failed to start transient unit {unit_name}, parent is {parent}: {err}")]
    FailedTransient {
        err: dbus::Error,
        unit_name: String,
        parent: String,
    },
    #[error("failed to stop unit {unit_name}: {err}")]
    FailedStop { err: dbus::Error, unit_name: String },
    #[error("failed to set properties for unit {unit_name}: {err}")]
    FailedProperties { err: dbus::Error, unit_name: String },
    #[error("could not parse systemd version: {0}")]
    SystemdVersion(ParseIntError),
}

pub trait SystemdClient {
    fn is_system(&self) -> bool;

    fn transient_unit_exists(&self, unit_name: &str) -> bool;

    fn start_transient_unit(
        &self,
        container_name: &str,
        pid: u32,
        parent: &str,
        unit_name: &str,
    ) -> Result<(), SystemdClientError>;

    fn stop_transient_unit(&self, unit_name: &str) -> Result<(), SystemdClientError>;

    fn set_unit_properties(
        &self,
        unit_name: &str,
        properties: &HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<(), SystemdClientError>;

    fn systemd_version(&self) -> Result<u32, SystemdClientError>;

    fn control_cgroup_root(&self) -> Result<PathBuf, SystemdClientError>;
}

/// Client is a wrapper providing higher level API and abatraction around dbus.
/// For more information see https://www.freedesktop.org/wiki/Software/systemd/dbus/
pub struct Client {
    conn: Connection,
    system: bool,
}

impl Client {
    /// Uses the system bus to communicate with systemd
    pub fn new_system() -> Result<Self, dbus::Error> {
        let conn = Connection::new_system()?;
        Ok(Client { conn, system: true })
    }

    /// Uses the session bus to communicate with systemd
    pub fn new_session() -> Result<Self, dbus::Error> {
        let conn = Connection::new_session()?;
        Ok(Client {
            conn,
            system: false,
        })
    }

    fn create_proxy(&self) -> Proxy<&Connection> {
        self.conn.with_proxy(
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            Duration::from_millis(5000),
        )
    }
}

impl SystemdClient for Client {
    fn is_system(&self) -> bool {
        self.system
    }

    fn transient_unit_exists(&self, unit_name: &str) -> bool {
        let proxy = self.create_proxy();
        proxy.get_unit(unit_name).is_ok()
    }

    /// start_transient_unit is a higher level API for starting a unit
    /// for a specific container under systemd.
    /// See https://www.freedesktop.org/wiki/Software/systemd/dbus for more details.
    fn start_transient_unit(
        &self,
        container_name: &str,
        pid: u32,
        parent: &str,
        unit_name: &str,
    ) -> Result<(), SystemdClientError> {
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
            Variant(Box::new(format!("youki container {container_name}"))),
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

        tracing::debug!("Starting transient unit: {:?}", properties);
        proxy
            .start_transient_unit(unit_name, "replace", properties, vec![])
            .map_err(|err| SystemdClientError::FailedTransient {
                err,
                unit_name: unit_name.into(),
                parent: parent.into(),
            })?;
        Ok(())
    }

    fn stop_transient_unit(&self, unit_name: &str) -> Result<(), SystemdClientError> {
        let proxy = self.create_proxy();

        proxy
            .stop_unit(unit_name, "replace")
            .map_err(|err| SystemdClientError::FailedStop {
                err,
                unit_name: unit_name.into(),
            })?;
        Ok(())
    }

    fn set_unit_properties(
        &self,
        unit_name: &str,
        properties: &HashMap<&str, Box<dyn RefArg>>,
    ) -> Result<(), SystemdClientError> {
        let proxy = self.create_proxy();

        let props = properties
            .iter()
            .map(|p| (*p.0, Variant(p.1.box_clone())))
            .collect();

        proxy
            .set_unit_properties(unit_name, true, props)
            .map_err(|err| SystemdClientError::FailedProperties {
                err,
                unit_name: unit_name.into(),
            })?;
        Ok(())
    }

    fn systemd_version(&self) -> Result<u32, SystemdClientError> {
        let proxy = self.create_proxy();

        let version = proxy
            .version()?
            .chars()
            .skip_while(|c| c.is_alphabetic())
            .take_while(|c| c.is_numeric())
            .collect::<String>()
            .parse::<u32>()
            .map_err(SystemdClientError::SystemdVersion)?;

        Ok(version)
    }

    fn control_cgroup_root(&self) -> Result<PathBuf, SystemdClientError> {
        let proxy = self.create_proxy();

        let cgroup_root = proxy.control_group()?;
        Ok(PathBuf::from(&cgroup_root))
    }
}
