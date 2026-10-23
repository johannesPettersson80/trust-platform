use alloc::string::String;
use core::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;

/// One control request: `{"id", "type", "params", "auth", "request_id"}`. Members the envelope
/// does not name are ignored.
#[derive(Debug, Deserialize)]
pub struct ControlRequest {
    /// Request number, echoed in the response.
    pub id: u64,
    /// Operation name, for example `status`.
    #[serde(rename = "type")]
    pub r#type: String,
    /// Operation parameters.
    pub params: Option<serde_json::Value>,
    /// Control token or pairing token.
    pub auth: Option<String>,
    /// Caller's correlation id for the audit record; `correlation_id` is accepted as an alias.
    #[serde(default, alias = "correlation_id")]
    pub request_id: Option<String>,
}

/// Why a request line is not a request (specification 11, §6.8.1).
#[derive(Debug)]
pub(crate) enum LineError {
    /// The line is not JSON: the error of reading it as a JSON value.
    Unreadable(serde_json::Error),
    /// The line is JSON but not a request: the error of reading the envelope from its value.
    NotARequest(serde_json::Error),
}

/// Reads one request line (specification 11, §6.8.1).
///
/// The line is read straight into the envelope, which builds no JSON value of the whole line: on
/// the ESP32-C6 that value alone took 0.35 ms for the shortest request (23 October 2026). A line that
/// this reading does not take is read as it always was, as a JSON value and then as the envelope, so
/// that every line is a request, or is refused with its error, exactly as those two readings decide.
pub(crate) fn read_line(line: &str) -> Result<ControlRequest, LineError> {
    if let Ok(Envelope(request)) = serde_json::from_str::<Envelope>(line) {
        return Ok(request);
    }
    let value = serde_json::from_str::<serde_json::Value>(line).map_err(LineError::Unreadable)?;
    serde_json::from_value(value).map_err(LineError::NotARequest)
}

/// The envelope read straight from a line, the first reading of [`read_line`]. It takes only an
/// object that the two readings take with the same result: `id` and `type`, and `params`, `auth`
/// and one of `request_id` and `correlation_id` when they are given, each at most once, and every
/// other member read as a JSON value, as the value of the whole line reads it, and dropped. Any
/// other line it leaves to the two readings.
struct Envelope(ControlRequest);

/// A member of the envelope, by its name with its escapes read.
enum Member {
    Id,
    Type,
    Params,
    Auth,
    RequestId,
    Other,
}

impl<'de> Deserialize<'de> for Member {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_identifier(MemberVisitor)
    }
}

struct MemberVisitor;

impl Visitor<'_> for MemberVisitor {
    type Value = Member;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a member name")
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<Member, E> {
        Ok(match name {
            "id" => Member::Id,
            "type" => Member::Type,
            "params" => Member::Params,
            "auth" => Member::Auth,
            "request_id" | "correlation_id" => Member::RequestId,
            _ => Member::Other,
        })
    }
}

impl<'de> Deserialize<'de> for Envelope {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(EnvelopeVisitor)
    }
}

struct EnvelopeVisitor;

/// Stores the value of a member in `slot`, which holds it once it is given; a second one is an
/// error, as it is to the envelope's reading from a value.
fn once<T, E: de::Error>(slot: &mut Option<T>, value: T) -> Result<(), E> {
    if slot.is_some() {
        return Err(E::custom("a member given twice"));
    }
    *slot = Some(value);
    Ok(())
}

impl<'de> Visitor<'de> for EnvelopeVisitor {
    type Value = Envelope;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a control request object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Envelope, A::Error> {
        let mut id = None;
        let mut kind = None;
        // The optional members, `Some` once given, `null` included.
        let mut params: Option<Option<serde_json::Value>> = None;
        let mut auth: Option<Option<String>> = None;
        let mut request_id: Option<Option<String>> = None;
        while let Some(member) = map.next_key::<Member>()? {
            match member {
                Member::Id => once(&mut id, map.next_value::<u64>()?)?,
                Member::Type => once(&mut kind, map.next_value::<String>()?)?,
                Member::Params => once(&mut params, map.next_value()?)?,
                Member::Auth => once(&mut auth, map.next_value()?)?,
                Member::RequestId => once(&mut request_id, map.next_value()?)?,
                Member::Other => {
                    map.next_value::<serde_json::Value>()?;
                }
            }
        }
        let (Some(id), Some(r#type)) = (id, kind) else {
            return Err(de::Error::custom("a request without its id or type"));
        };
        Ok(Envelope(ControlRequest {
            id,
            r#type,
            params: params.flatten(),
            auth: auth.flatten(),
            request_id: request_id.flatten(),
        }))
    }
}
