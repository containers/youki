use super::dbus::DbusConnection;
use super::message::*;
use super::serialize::DbusSerialize;
use super::utils::{Result, SystemdClientError};

/// Structure to conveniently communicate with
/// given destination and path for method calls
pub struct Proxy<'conn> {
    conn: &'conn mut DbusConnection,
    dest: String,
    path: String,
}

impl<'conn> Proxy<'conn> {
    /// create a new proxy for given destination and path over given connection
    pub fn new(conn: &'conn mut DbusConnection, dest: String, path: String) -> Self {
        Self { conn, dest, path }
    }

    /// Do a method call for given interface and member by sending given body
    /// If no body is to be sent, set it as `None`
    pub fn method_call<Body: DbusSerialize, Output: DbusSerialize>(
        &mut self,
        interface: &str,
        member: &str,
        body: Option<Body>,
    ) -> Result<Output> {
        let mut headers = Vec::with_capacity(4);

        // create necessary headers
        headers.push(Header {
            kind: HeaderKind::Path,
            value: HeaderValue::String(self.path.clone()),
        });
        headers.push(Header {
            kind: HeaderKind::Destination,
            value: HeaderValue::String(self.dest.clone()),
        });
        headers.push(Header {
            kind: HeaderKind::Interface,
            value: HeaderValue::String(interface.to_string()),
        });
        headers.push(Header {
            kind: HeaderKind::Member,
            value: HeaderValue::String(member.to_string()),
        });

        let mut serialized_body = vec![];

        // if there is some body, serialize it, and set the
        // body signature header accordingly
        if let Some(v) = body {
            headers.push(Header {
                kind: HeaderKind::BodySignature,
                value: HeaderValue::String(Body::get_signature()),
            });
            v.serialize(&mut serialized_body);
        }

        // send the message and get response
        let reply_messages =
            self.conn
                .send_message(MessageType::MethodCall, headers, serialized_body)?;

        // check if there is any error message
        let error_message: Vec<_> = reply_messages
            .iter()
            .filter(|m| m.preamble.mtype == MessageType::Error)
            .collect();

        // if any error, return error
        if !error_message.is_empty() {
            let msg = error_message[0];
            if msg.body.is_empty() {
                // this should rarely be the case
                return Err(SystemdClientError::MethodCallErr(
                    "Unknown Dbus Error".into(),
                ));
            } else {
                // in error message, first item of the body (if present) is always a string
                // indicating the error
                let mut ctr = 0;
                return Err(SystemdClientError::MethodCallErr(String::deserialize(
                    &msg.body, &mut ctr,
                )?));
            }
        }

        // we basically ignore all type of messages apart from method return
        let reply: Vec<_> = reply_messages
            .iter()
            .filter(|m| m.preamble.mtype == MessageType::MethodReturn)
            .collect();

        // we are only going to consider first reply, cause... so.
        // realistically there should only be at most one method return type of message
        // for a method call
        let reply = reply[0];

        let headers = &reply.headers;
        let expected_signature = Output::get_signature();

        // get the signature header
        let signature_header: Vec<_> = headers
            .iter()
            .filter(|h| h.kind == HeaderKind::BodySignature)
            .collect();

        // This is also something that should never happen
        // we just check this defensively
        if signature_header.is_empty() && !reply.body.is_empty() {
            return Err(SystemdClientError::MethodCallErr(
                "Body non empty, but body signature header missing".to_string(),
            ));
        }

        if expected_signature == *"" {
            // This is for the case when there is no body, i.e. Output = ()
            // we must do this as the signature header will be
            // absent in that case, so instead we choose to
            // parse and return early
            // This is a bit hacky, but works
            let mut ctr = 0;
            return Output::deserialize(&[], &mut ctr);
        }

        let actual_signature = match &signature_header[0].value {
            HeaderValue::String(s) => s,
            _ => unreachable!("body signature header will always be string type"),
        };

        // check that signature returned and type we are trying to deserialize
        // match as expected
        if *actual_signature != expected_signature {
            return Err(SystemdClientError::DeserializationError(format!(
                "reply signature mismatch : expected {}, found {}",
                expected_signature, actual_signature
            )));
        }

        let mut ctr = 0;
        Output::deserialize(&reply.body, &mut ctr)
    }
}
