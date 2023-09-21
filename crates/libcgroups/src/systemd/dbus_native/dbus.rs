use crate::systemd::dbus_native::serialize::{DbusSerialize, Structure, Variant};

use super::client::SystemdClient;
use super::message::*;
use super::proxy::Proxy;
use super::utils::{Result, SystemdClientError};
use nix::sys::socket;
use std::collections::HashMap;
use std::io::{IoSlice, IoSliceMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

const REPLY_BUF_SIZE: usize = 128; // seems good enough tradeoff between extra size and repeated calls

/// NOTE that this is meant for a single-threaded use, and concurrent
/// usage can cause errors, primarily because then the message received over
/// socket can be out of order and we need to manager buffer and check with message counter
/// which message is for which request etc etc
// Client is a wrapper providing higher level API and abatraction around dbus.
// For more information see https://www.freedesktop.org/wiki/Software/systemd/dbus/
pub struct DbusConnection {
    /// Is the socket system level or session specific
    system: bool,
    /// socket fd
    socket: i32,
    /// counter for messages
    // This must be atomic, so that we can take non-mutable reference to self
    // and still increment this
    msg_ctr: AtomicU32,
}

#[inline(always)]
fn uid_to_hex_str(uid: u32) -> String {
    let temp: Vec<_> = uid
        .to_string()
        .chars()
        .map(|c| format!("{:x}", c as u8))
        .collect();
    temp.join("")
}

impl DbusConnection {
    /// Open a new dbus connection to given address
    /// authenticating as user with given uid
    pub fn new(addr: &str, uid: u32, system: bool) -> Result<Self> {
        let socket = socket::socket(
            socket::AddressFamily::Unix,
            socket::SockType::Stream,
            socket::SockFlag::empty(),
            None,
        )?;

        let addr = socket::UnixAddr::new(addr)?;
        socket::connect(socket, &addr)?;
        let dbus = Self {
            socket,
            msg_ctr: AtomicU32::new(0),
            system,
        };
        dbus.authenticate(uid)?;
        Ok(dbus)
    }

    pub fn new_system() -> Result<Self> {
        todo!()
    }

    pub fn new_session() -> Result<Self> {
        todo!();
    }

    /// Authenticates with dbus using given uid via external strategy
    /// Must be called on any connection before doing any other communication
    fn authenticate(&self, uid: u32) -> Result<()> {
        let mut buf = [0; 64];

        // dbus connection always start with a 0 byte sent as first thing
        socket::send(self.socket, &[0], socket::MsgFlags::empty())?;

        let msg = format!("AUTH EXTERNAL {}\r\n", uid_to_hex_str(uid));

        // then we send our auth with uid
        socket::send(self.socket, msg.as_bytes(), socket::MsgFlags::empty())?;

        // we get the reply and check if all went well or not
        socket::recv(self.socket, &mut buf, socket::MsgFlags::empty())?;

        let reply: Vec<u8> = buf.iter().filter(|v| **v != 0).copied().collect();

        // we can use _lossy as we know dbus communication is always ascii
        let reply = String::from_utf8_lossy(&reply);

        // successful auth reply starts with 'ok'
        if !reply.starts_with("OK") {
            return Err(SystemdClientError::AuthenticationErr(format!(
                "Authentication failed, got message : {}",
                reply
            )));
        }

        // we must send the BEGIN before starting any actual communication
        // we can also send AGREE_UNIX_FD before this if we need to deal with sending/receiving
        // fds over the connection, but because youki doesn't need it, we can skip that
        socket::send(
            self.socket,
            "BEGIN\r\n".as_bytes(),
            socket::MsgFlags::empty(),
        )?;

        // First thing any dbus client must do after authentication
        // is to do a hello method call, in order to get a name allocated
        // if we do any other method call, the connection is assumed to be
        // invalid and auto disconnected
        let headers = vec![
            Header {
                kind: HeaderKind::Path,
                value: HeaderValue::String("/org/freedesktop/DBus".to_string()),
            },
            Header {
                kind: HeaderKind::Destination,
                value: HeaderValue::String("org.freedesktop.DBus".to_string()),
            },
            Header {
                kind: HeaderKind::Interface,
                value: HeaderValue::String("org.freedesktop.DBus".to_string()),
            },
            Header {
                kind: HeaderKind::Member,
                value: HeaderValue::String("Hello".to_string()),
            },
        ];

        self.send_message(MessageType::MethodCall, headers, vec![])?;

        Ok(())
    }

    /// Helper function to get complete message in chunks
    /// over the socket. This will loop and collect all of the message
    /// chunks into a single vector
    fn receive_complete_response(&self) -> Result<Vec<u8>> {
        let mut ret = Vec::with_capacity(512);
        loop {
            let mut reply: [u8; REPLY_BUF_SIZE] = [0_u8; REPLY_BUF_SIZE];
            let reply_buffer = IoSliceMut::new(&mut reply[0..]);

            let reply_rcvd = socket::recvmsg::<()>(
                self.socket,
                &mut [reply_buffer],
                None,
                socket::MsgFlags::empty(),
            )?;

            let received_byte_count = reply_rcvd.bytes;

            ret.extend_from_slice(&reply[0..received_byte_count]);

            if received_byte_count < REPLY_BUF_SIZE {
                // if received byte count is less than buffer size, then we got all
                break;
            }
        }
        Ok(ret)
    }

    /// function to send message of given type with given headers and body
    /// over the dbus connection. The caller must specify the destination, interface etc.etc.
    /// in the headers, this function will only take care of sending the message and
    /// returning the received messages. Note that the caller must check if any error
    /// message was returned or not, this will not check that, the returned Err
    /// indicates error in sending/receiving message
    pub fn send_message(
        &self,
        mtype: MessageType,
        headers: Vec<Header>,
        body: Vec<u8>,
    ) -> Result<Vec<Message>> {
        let message = Message::new(mtype, self.get_msg_id(), headers, body);
        let serialized = message.serialize();

        socket::sendmsg::<()>(
            self.socket,
            &[IoSlice::new(&serialized)],
            &[],
            socket::MsgFlags::empty(),
            None,
        )?;

        let reply = self.receive_complete_response()?;

        // note that a single received response can contain multiple
        // messages, so we must deserialize it piece by piece
        let mut ret = Vec::new();
        let mut buf = &reply[..];

        while !buf.is_empty() {
            let mut ctr = 0;
            let msg = Message::deserialize(&buf[ctr..], &mut ctr)?;
            // we reset the buf, because I couldn't figure out how the adjust_counter function
            // should should be changed to work correctly with non-zero start counter, and this solved that issue
            buf = &buf[ctr..];
            ret.push(msg);
        }
        Ok(ret)
    }

    /// function to manage the message counter
    fn get_msg_id(&self) -> u32 {
        let old_ctr = self.msg_ctr.fetch_add(1, Ordering::SeqCst);
        old_ctr + 1
    }

    /// Create a proxy for given destination and path
    pub fn proxy(&self, destination: &str, path: &str) -> Proxy {
        Proxy::new(self, destination, path)
    }

    fn create_proxy(&self) -> Proxy {
        self.proxy("org.freedesktop.systemd1", "/org/freedesktop/systemd1")
    }
}

impl SystemdClient for DbusConnection {
    fn is_system(&self) -> bool {
        self.system
    }

    fn transient_unit_exists(&self, unit_name: &str) -> bool {
        let mut proxy = self.create_proxy();
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
        let mut properties: Vec<(&str, Variant)> = Vec::with_capacity(6);
        properties.push((
            "Description",
            Variant::String(format!("youki container {container_name}")),
        ));

        // if we create a slice, the parent is defined via a Wants=
        // otherwise, we use Slice=
        if unit_name.ends_with("slice") {
            properties.push(("Wants", Variant::String(parent.to_owned())));
        } else {
            properties.push(("Slice", Variant::String(parent.to_owned())));
            properties.push(("Delegate", Variant::Bool(true)));
        }

        properties.push(("MemoryAccounting", Variant::Bool(true)));
        properties.push(("CPUAccounting", Variant::Bool(true)));
        properties.push(("IOAccounting", Variant::Bool(true)));
        properties.push(("TasksAccounting", Variant::Bool(true)));

        properties.push(("DefaultDependencies", Variant::Bool(false)));
        properties.push(("PIDs", Variant::ArrayU32(vec![pid])));

        tracing::debug!("Starting transient unit: {:?}", properties);
        let props = properties
            .into_iter()
            .map(|(k, v)| Structure::new(k.into(), v))
            .collect();
        proxy
            .start_transient_unit(unit_name, "replace", props, vec![])
            .map_err(|err| SystemdClientError::FailedTransient {
                err: Box::new(err),
                unit_name: unit_name.into(),
                parent: parent.into(),
            })?;
        Ok(())
    }

    fn stop_transient_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.create_proxy();

        proxy
            .stop_unit(unit_name, "replace")
            .map_err(|err| SystemdClientError::FailedStop {
                err: Box::new(err),
                unit_name: unit_name.into(),
            })?;
        Ok(())
    }

    fn set_unit_properties(
        &self,
        unit_name: &str,
        properties: &HashMap<&str, Variant>,
    ) -> Result<()> {
        let proxy = self.create_proxy();

        let props: Vec<Structure<Variant>> = properties
            .iter()
            .map(|(k, v)| Structure::new(k.to_string(), v.clone()))
            .collect();

        proxy
            .set_unit_properties(unit_name, true, props)
            .map_err(|err| SystemdClientError::FailedProperties {
                err: Box::new(err),
                unit_name: unit_name.into(),
            })?;
        Ok(())
    }

    fn systemd_version(&self) -> std::result::Result<u32, SystemdClientError> {
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

    fn control_cgroup_root(&self) -> std::result::Result<PathBuf, SystemdClientError> {
        let proxy = self.create_proxy();

        let cgroup_root = proxy.control_group()?;
        Ok(PathBuf::from(&cgroup_root))
    }
}

#[cfg(test)]
mod tests {
    use super::super::utils::Result;
    use super::{uid_to_hex_str, DbusConnection, SystemdClientError};
    use nix::unistd::getuid;

    #[test]
    fn test_uid_to_hex_str() {
        let uid0 = uid_to_hex_str(0);
        assert_eq!(uid0, "30");
        let uid1000 = uid_to_hex_str(1000);
        assert_eq!(uid1000, "31303030");
    }

    #[test]
    #[cfg(feature = "systemd")]
    fn test_dbus_connection_auth() {
        let uid: u32 = getuid().into();

        let dbus_pipe_path = format!("/run/user/{}/bus", uid);

        let conn = DbusConnection::new(&dbus_pipe_path, uid, false);
        assert!(conn.is_ok());

        let invalid_conn = DbusConnection::new(&dbus_pipe_path, uid.wrapping_add(1), false);
        assert!(invalid_conn.is_err());
    }

    #[test]
    #[cfg(feature = "systemd")]
    fn test_dbus_function_calls() -> Result<()> {
        use crate::systemd::dbus_native::serialize::Variant;

        let uid: u32 = getuid().into();

        let dbus_pipe_path = format!("/run/user/{}/bus", uid);

        let conn = DbusConnection::new(&dbus_pipe_path, uid, false)?;

        let proxy = conn.proxy("org.freedesktop.systemd1", "/org/freedesktop/systemd1");

        let body = (
            "org.freedesktop.systemd1.Manager".to_string(),
            "Version".to_string(),
        );
        let t = proxy.method_call::<_, Variant>(
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(body),
        )?;
        assert!(matches!(t, Variant::String(_)));

        let body = (
            "org.freedesktop.systemd1.Manager".to_string(),
            "ControlGroup".to_string(),
        );
        let t = proxy.method_call::<_, Variant>(
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(body),
        )?;
        assert!(matches!(t, Variant::String(_)));

        Ok(())
    }

    #[test]
    #[cfg(feature = "systemd")]
    fn test_dbus_function_calls_errors() {
        let uid: u32 = getuid().into();

        let dbus_pipe_path = format!("/run/user/{}/bus", uid);

        let conn = DbusConnection::new(&dbus_pipe_path, uid, false).unwrap();

        let proxy = conn.proxy("org.freedesktop.systemd1", "/org/freedesktop/systemd1");
        let body = (
            "org.freedesktop.systemd1.Manager".to_string(),
            "ControlGroup".to_string(),
        );

        // invalid return type, this call returns variant<String>
        let res = proxy.method_call::<_, u16>("org.freedesktop.DBus.Properties", "Get", Some(body));
        assert!(res.is_err());
        assert!(matches!(
            res,
            Err(SystemdClientError::DeserializationError(_))
        ));

        let body = (
            "org.freedesktop.systemd1.Manager".to_string(),
            "ControlGroup".to_string(),
        );

        // invalid interface
        let res = proxy.method_call::<_, u16>("org.freedesktop.DBus.Propertie_", "Get", Some(body));
        assert!(res.is_err());
        assert!(matches!(res, Err(SystemdClientError::MethodCallErr(_))))
    }
}
