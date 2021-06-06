use anyhow::Result;
use dbus::blocking::Connection;
use std::time::Duration;
use std::vec::Vec;

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

    /// start_unit starts a specific unit under systemd. See https://www.freedesktop.org/wiki/Software/systemd/dbus
    /// for more details.
    pub fn start_unit(&self, unit_name: &str, _properties: Vec<&str>) -> Result<()> {
        let proxy = self.conn.with_proxy(
            "org.freedesktop.systemd1.Manager",
            "/",
            Duration::from_millis(5000),
        );
        let (_job_id,): (i32,) = proxy.method_call(
            "org.freedesktop.systemd1.Manager",
            "StartTransientUnit",
            (unit_name, "replace"),
        )?;
        Ok(())
    }
}
