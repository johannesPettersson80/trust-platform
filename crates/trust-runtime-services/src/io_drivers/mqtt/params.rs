//! The `mqtt` driver's `params` table (specification 32 §2 to §9) with the keys, defaults and
//! refusals of trust-runtime's driver. A refusal names the key and the reason; no reason holds a
//! password, a user name or a broker's user information (specification 32 §3).

use alloc::format;
use alloc::vec::Vec;
use core::fmt;

use serde_json::Value;
use smol_str::SmolStr;

use super::points::{self, InputPoint, OutputPoint};
use super::sparkplug::{self, SparkplugParams};
use crate::io_drivers::{parse_root, quoted, OnError, ParamsError, Table};

/// The raw input topic when `topic_in` is absent (specification 32 §4).
const DEFAULT_TOPIC_IN: &str = "trust/io/in";

/// The raw output topic when `topic_out` is absent.
const DEFAULT_TOPIC_OUT: &str = "trust/io/out";

/// `reconnect_ms` when absent.
const DEFAULT_RECONNECT_MS: u64 = 500;

/// `keep_alive_s` when absent.
const DEFAULT_KEEP_ALIVE_S: u16 = 5;

/// The largest integer that the TOML of `io.toml` and a record's JSON carry, `i64::MAX`.
pub(super) const MAX_JSON_INTEGER: u64 = 9_223_372_036_854_775_807;

/// The longest MQTT string, whose length takes two bytes.
const MAX_STRING: usize = 65_535;

/// The keys of the table, which a refusal of another key lists.
const KEYS: [&str; 19] = [
    "allow_insecure_remote",
    "broker",
    "client_id",
    "input_points",
    "keep_alive_s",
    "mappings",
    "on_error",
    "output_points",
    "password",
    "reconnect_ms",
    "sparkplug",
    "tls",
    "tls_alpn",
    "tls_ca_path",
    "tls_client_cert_path",
    "tls_client_key_path",
    "topic_in",
    "topic_out",
    "username",
];

/// What `broker` must be.
const BROKER: &str = "the broker's host:port, such as \"mqtt://127.0.0.1:1883\"";

/// What a mapping's `tag` must be.
const TAG: &str = "the fully qualified name of a program variable, such as \"MainInstance.Green\"";

/// What a mapping's `direction` must be.
const DIRECTION: &str = "\"read\", from the broker to the program, or \"write\", from the program \
                         to the broker";

/// The `mqtt` driver's parameters (specification 32).
#[derive(Debug, Clone, PartialEq)]
pub struct MqttParams {
    /// `broker`: the broker's host and port; required.
    pub broker: BrokerEndpoint,
    /// `allow_insecure_remote`: a broker other than `localhost`, `127.0.0.1` and `::1` is taken
    /// without TLS; default false.
    pub allow_insecure_remote: bool,
    /// `client_id`; `None` takes the runtime's default, `trust-runtime-<process id>` on a computer.
    pub client_id: Option<SmolStr>,
    /// `topic_in`, trimmed: the raw input topic; default `trust/io/in`.
    pub topic_in: SmolStr,
    /// `topic_out`, trimmed: the raw output topic; default `trust/io/out`.
    pub topic_out: SmolStr,
    /// `username` and `password`, given together.
    pub credentials: Option<Credentials>,
    /// `reconnect_ms`: the backoff after a connection failure, at least 1 ms; default 500.
    pub reconnect_ms: u64,
    /// `keep_alive_s`, 1 to 65,535 s, a given 0 read as 1; default 5.
    pub keep_alive_s: u16,
    /// `on_error`; default `fault`.
    pub on_error: OnError,
    /// TLS, on with `tls = true` or an `mqtts://` or `ssl://` broker.
    pub tls: Option<MqttTls>,
    /// `mappings` when the key is given, even empty: its presence selects mapped mode
    /// (specification 32 §4). The runtime lowers the mappings to typed points before the driver
    /// starts (§5); the driver exchanges those points.
    pub mappings: Option<Vec<Mapping>>,
    /// `input_points`, in `io.toml` order.
    pub input_points: Vec<InputPoint>,
    /// `output_points`, in `io.toml` order.
    pub output_points: Vec<OutputPoint>,
    /// `sparkplug` when its profile is enabled.
    pub sparkplug: Option<SparkplugParams>,
}

/// A broker's host and port (specification 32 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerEndpoint {
    /// The host name or address, without the brackets of an IPv6 address; also the name that TLS
    /// verifies.
    pub host: SmolStr,
    /// The port, 1 to 65,535.
    pub port: u16,
}

/// `username` and `password`. Its `Debug` writes the password as `<redacted>`.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    /// The user name, not empty.
    pub username: SmolStr,
    /// The password; an empty one is not sent.
    pub password: SmolStr,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// The TLS settings (specification 32 §3). The runtime reads the files at the paths; the broker's
/// host is the name it verifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttTls {
    /// `tls_ca_path`, trimmed: the PEM file of the broker's certificate authority.
    pub ca_path: SmolStr,
    /// `tls_client_cert_path` and `tls_client_key_path`, for mutual TLS.
    pub client_auth: Option<ClientAuth>,
    /// `tls_alpn`, each protocol trimmed; `Some` and empty for an empty list.
    pub alpn: Option<Vec<SmolStr>>,
}

/// The client's certificate and key for mutual TLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAuth {
    /// `tls_client_cert_path`, trimmed.
    pub cert_path: SmolStr,
    /// `tls_client_key_path`, trimmed.
    pub key_path: SmolStr,
}

/// A symbolic mapping (specification 32 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// `tag`: a program instance's scalar variable, such as `MainInstance.Green`.
    pub tag: SmolStr,
    /// `topic`, trimmed: an exact topic.
    pub topic: SmolStr,
    /// `direction`.
    pub direction: MappingDirection,
}

/// A mapping's direction, as the PLC sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingDirection {
    /// `read`: the broker's value to the PLC.
    Read,
    /// `write`: the PLC's value to the broker.
    Write,
}

impl MqttParams {
    /// The parameters of the JSON text `text`, the driver's `params` table (specification 34
    /// §5.1.1, section 8), checked by the rules of specification 32. The paths of the TLS files
    /// are checked as text; the runtime that reads the files refuses one it cannot read.
    ///
    /// # Errors
    ///
    /// The first key that breaks a rule, with the reason.
    pub fn from_json(text: &str) -> Result<Self, ParamsError> {
        let map = parse_root(text)?;
        let root = Table::root(&map);
        root.only(&KEYS, "the mqtt driver")?;
        let (broker, scheme_tls) = broker(&root)?;
        let tls = tls(&root, scheme_tls)?;
        let allow_insecure_remote = root.boolean("allow_insecure_remote")?.unwrap_or(false);
        if tls.is_none() && !allow_insecure_remote && !is_local_host(&broker.host) {
            return Err(root.refuse(
                "allow_insecure_remote",
                format!(
                    "must be true for the remote broker {} without TLS; only localhost, \
                     127.0.0.1 and ::1 are local",
                    quoted(&broker.host)
                ),
            ));
        }
        let credentials = credentials(&root)?;
        let client_id = client_id(&root)?;
        let topic_in = raw_topic(&root, "topic_in", DEFAULT_TOPIC_IN)?;
        let topic_out = raw_topic(&root, "topic_out", DEFAULT_TOPIC_OUT)?;
        let reconnect_ms = root
            .integer("reconnect_ms", 0_u64, MAX_JSON_INTEGER)?
            .unwrap_or(DEFAULT_RECONNECT_MS)
            .max(1);
        let keep_alive_s = root
            .integer("keep_alive_s", 0_u16, u16::MAX)?
            .unwrap_or(DEFAULT_KEEP_ALIVE_S)
            .max(1);
        let on_error = on_error(&root)?;
        let mappings = mappings(&root)?;
        let input_points = points::input_points(&root)?;
        let output_points = points::output_points(&root)?;
        let sparkplug = sparkplug::params(&root)?;
        if sparkplug.is_some() {
            sparkplug::check_profile(&root, &input_points, &output_points)?;
        }
        Ok(Self {
            broker,
            allow_insecure_remote,
            client_id,
            topic_in,
            topic_out,
            credentials,
            reconnect_ms,
            keep_alive_s,
            on_error,
            tls,
            mappings,
            input_points,
            output_points,
            sparkplug,
        })
    }

    /// Whether the driver reads inputs: always without `mappings`, and with them only when it has
    /// input points (specification 32 §4).
    #[must_use]
    pub fn input_enabled(&self) -> bool {
        self.mappings.is_none() || !self.input_points.is_empty()
    }

    /// Whether the driver writes outputs: always without `mappings`, and with them only when it
    /// has output points.
    #[must_use]
    pub fn output_enabled(&self) -> bool {
        self.mappings.is_none() || !self.output_points.is_empty()
    }

    /// The topics the driver subscribes to: none without inputs, `topic_in` in raw mode, and each
    /// input point's topic otherwise, in order.
    #[must_use]
    pub fn subscribe_topics(&self) -> Vec<SmolStr> {
        if !self.input_enabled() {
            return Vec::new();
        }
        if self.input_points.is_empty() {
            return alloc::vec![self.topic_in.clone()];
        }
        self.input_points
            .iter()
            .map(|point| point.topic.clone())
            .collect()
    }

    /// Whether `other` has the authentication context of `self`, under which a redacted edit may
    /// keep a stored password (specification 32 §3): the broker's host and port, the TLS paths and
    /// ALPN, the user name, the client identifier and `allow_insecure_remote`, defaults included.
    /// Whether the files at unchanged paths changed is for the runtime that reads them.
    #[must_use]
    pub fn same_credential_context(&self, other: &Self) -> bool {
        let username = |params: &Self| {
            params
                .credentials
                .as_ref()
                .map(|credentials| credentials.username.clone())
        };
        self.broker == other.broker
            && self.tls == other.tls
            && username(self) == username(other)
            && self.client_id == other.client_id
            && self.allow_insecure_remote == other.allow_insecure_remote
    }
}

/// `broker`: `host:port` after an optional `mqtt://` or `tcp://` (plain) or `mqtts://` or
/// `ssl://` (TLS) scheme, and the host of an IPv6 address in brackets; with whether the scheme is
/// TLS. A broker with user information is refused without its text.
fn broker(root: &Table<'_>) -> Result<(BrokerEndpoint, bool), ParamsError> {
    let Some(value) = root.get("broker") else {
        return Err(root.refuse("broker", format!("is required, {BROKER}")));
    };
    let Some(text) = value.as_str() else {
        return Err(root.refuse("broker", format!("must be a string, {BROKER}, not {value}")));
    };
    if text.contains('@') {
        return Err(root.refuse(
            "broker",
            "must not carry user information: give username and password instead",
        ));
    }
    let shown = quoted(text);
    let trimmed = text.trim();
    let (rest, scheme_tls) = if let Some(rest) = trimmed
        .strip_prefix("tcp://")
        .or_else(|| trimmed.strip_prefix("mqtt://"))
    {
        (rest, false)
    } else if let Some(rest) = trimmed
        .strip_prefix("ssl://")
        .or_else(|| trimmed.strip_prefix("mqtts://"))
    {
        (rest, true)
    } else {
        (trimmed, false)
    };
    if rest.contains("://") {
        return Err(root.refuse(
            "broker",
            format!("must use mqtt://, tcp://, mqtts:// or ssl://, or no scheme, not {shown}"),
        ));
    }
    let refuse_shape = || root.refuse("broker", format!("must be host:port, not {shown}"));
    let (host, port) = if let Some(inner) = rest.strip_prefix('[') {
        inner.split_once("]:").ok_or_else(refuse_shape)?
    } else {
        let (host, port) = rest.rsplit_once(':').ok_or_else(refuse_shape)?;
        (host.trim(), port)
    };
    if host.is_empty() {
        return Err(root.refuse("broker", format!("has an empty host: {shown}")));
    }
    let port = port.trim().parse::<u16>().map_err(|error| {
        root.refuse("broker", format!("has an invalid port in {shown}: {error}"))
    })?;
    if port == 0 {
        return Err(root.refuse(
            "broker",
            format!("has port 0 in {shown}; a port is from 1 to 65535"),
        ));
    }
    Ok((
        BrokerEndpoint {
            host: SmolStr::new(host),
            port,
        },
        scheme_tls,
    ))
}

/// Whether the broker `host` is local: `localhost` in any case, `127.0.0.1` or `::1`.
fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

/// The TLS settings: on with `tls = true` or a TLS scheme, which `tls = false` contradicts. With
/// TLS off, a path or a non-empty `tls_alpn` is refused; with it on, `tls_ca_path` is required and
/// the client's certificate and key come together (specification 32 §3).
fn tls(root: &Table<'_>, scheme_tls: bool) -> Result<Option<MqttTls>, ParamsError> {
    let flag = root.boolean("tls")?;
    if scheme_tls && flag == Some(false) {
        return Err(root.refuse(
            "tls",
            "is false, but the broker's mqtts:// or ssl:// scheme is TLS",
        ));
    }
    let ca = path(root, "tls_ca_path")?;
    let cert = path(root, "tls_client_cert_path")?;
    let key = path(root, "tls_client_key_path")?;
    let alpn = alpn(root)?;
    if !flag.unwrap_or(scheme_tls) {
        let given = [
            ("tls_ca_path", ca.is_some()),
            ("tls_client_cert_path", cert.is_some()),
            ("tls_client_key_path", key.is_some()),
            (
                "tls_alpn",
                alpn.as_ref().is_some_and(|list| !list.is_empty()),
            ),
        ];
        if let Some((field, _)) = given.iter().find(|(_, present)| *present) {
            return Err(root.refuse(
                field,
                "needs TLS: set tls = true or use an mqtts:// or ssl:// broker",
            ));
        }
        return Ok(None);
    }
    let Some(ca) = ca else {
        return Err(root.refuse(
            "tls_ca_path",
            "is required with TLS, the path of the PEM file of the broker's certificate authority",
        ));
    };
    let ca_path = filled(root, "tls_ca_path", ca)?;
    let client_auth = match (cert, key) {
        (None, None) => None,
        (Some(cert), Some(key)) => Some(ClientAuth {
            cert_path: filled(root, "tls_client_cert_path", cert)?,
            key_path: filled(root, "tls_client_key_path", key)?,
        }),
        (Some(_), None) => {
            return Err(root.refuse(
                "tls_client_key_path",
                "is required with tls_client_cert_path: mutual TLS takes both",
            ));
        }
        (None, Some(_)) => {
            return Err(root.refuse(
                "tls_client_cert_path",
                "is required with tls_client_key_path: mutual TLS takes both",
            ));
        }
    };
    let alpn = match alpn {
        None => None,
        Some(list) => Some(
            list.iter()
                .enumerate()
                .map(|(index, protocol)| {
                    let protocol = protocol.trim();
                    if protocol.is_empty() {
                        Err(root.refuse(&format!("tls_alpn[{index}]"), "must not be empty"))
                    } else {
                        Ok(SmolStr::new(protocol))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    Ok(Some(MqttTls {
        ca_path,
        client_auth,
        alpn,
    }))
}

/// The path `key` gives, when the key is given: any string, checked when TLS is on.
fn path<'a>(root: &Table<'a>, key: &str) -> Result<Option<&'a str>, ParamsError> {
    match root.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(other) => Err(root.refuse(
            key,
            format!("must be a string, the path of a PEM file, not {other}"),
        )),
    }
}

/// The path `given` of `key`, trimmed, which must not be empty.
fn filled(root: &Table<'_>, key: &str, given: &str) -> Result<SmolStr, ParamsError> {
    let trimmed = given.trim();
    if trimmed.is_empty() {
        return Err(root.refuse(key, "must not be empty"));
    }
    Ok(SmolStr::new(trimmed))
}

/// `tls_alpn`, when the key is given: an array of strings.
fn alpn<'a>(root: &Table<'a>) -> Result<Option<Vec<&'a str>>, ParamsError> {
    let Some(value) = root.get("tls_alpn") else {
        return Ok(None);
    };
    let refusal = || {
        root.refuse(
            "tls_alpn",
            format!("must be an array of strings, not {value}"),
        )
    };
    let Value::Array(items) = value else {
        return Err(refusal());
    };
    items
        .iter()
        .map(|item| item.as_str().ok_or_else(refusal))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// `username` and `password`, which come together; neither value appears in a refusal.
fn credentials(root: &Table<'_>) -> Result<Option<Credentials>, ParamsError> {
    let username = match root.get("username") {
        None => None,
        Some(Value::String(name))
            if !name.is_empty() && name.len() <= MAX_STRING && !name.contains('\0') =>
        {
            Some(name)
        }
        Some(_) => {
            return Err(root.refuse(
                "username",
                "must be a non-empty string of at most 65535 bytes without U+0000",
            ));
        }
    };
    let password = match root.get("password") {
        None => None,
        Some(Value::String(word)) if word.len() <= MAX_STRING => Some(word),
        Some(_) => {
            return Err(root.refuse("password", "must be a string of at most 65535 bytes"));
        }
    };
    match (username, password) {
        (Some(username), Some(password)) => Ok(Some(Credentials {
            username: SmolStr::new(username),
            password: SmolStr::new(password),
        })),
        (None, None) => Ok(None),
        (Some(_), None) => Err(root.refuse(
            "password",
            "is required with username: the two are given together",
        )),
        (None, Some(_)) => Err(root.refuse(
            "username",
            "is required with password: the two are given together",
        )),
    }
}

/// `client_id`, when the key is given: an MQTT string, which may be empty: the session is clean,
/// so a broker that takes an empty identifier gives the client one (MQTT-3.1.3-6).
fn client_id(root: &Table<'_>) -> Result<Option<SmolStr>, ParamsError> {
    match root.get("client_id") {
        None => Ok(None),
        Some(Value::String(id)) if id.len() <= MAX_STRING && !id.contains('\0') => {
            Ok(Some(SmolStr::new(id)))
        }
        Some(other) => Err(root.refuse(
            "client_id",
            format!("must be a string of at most 65535 bytes without U+0000, not {other}"),
        )),
    }
}

/// The raw topic `key`, an exact topic, or `default` when the key is absent.
fn raw_topic(root: &Table<'_>, key: &str, default: &str) -> Result<SmolStr, ParamsError> {
    Ok(points::exact_topic(root, key)?.unwrap_or_else(|| SmolStr::new(default)))
}

/// `on_error`, read as the Modbus driver reads it: trimmed, in any case, with `warning` for `warn`;
/// `fault` when absent (specification 11 §6.5).
fn on_error(root: &Table<'_>) -> Result<OnError, ParamsError> {
    let Some(value) = root.get("on_error") else {
        return Ok(OnError::default());
    };
    match value
        .as_str()
        .map(|text| text.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("fault") => Ok(OnError::Fault),
        Some("warn" | "warning") => Ok(OnError::Warn),
        Some("ignore") => Ok(OnError::Ignore),
        _ => Err(root.refuse(
            "on_error",
            format!("must be \"fault\", \"warn\" or \"ignore\", not {value}"),
        )),
    }
}

/// `mappings` when the key is given, even as an empty list: each mapping exactly `tag`, `topic`
/// and `direction` (specification 32 §5).
fn mappings(root: &Table<'_>) -> Result<Option<Vec<Mapping>>, ParamsError> {
    if root.get("mappings").is_none() {
        return Ok(None);
    }
    let mut mappings = Vec::new();
    for entry in root.list("mappings")? {
        entry.only(&["direction", "tag", "topic"], "a mapping")?;
        let tag = entry.text("tag", TAG)?;
        let tag = entry.required("tag", TAG, tag)?;
        let topic = points::exact_topic(&entry, "topic")?;
        let topic = entry.required("topic", points::EXACT_TOPIC, topic)?;
        let direction = entry.choice(
            "direction",
            &[
                ("read", MappingDirection::Read),
                ("write", MappingDirection::Write),
            ],
        )?;
        let direction = entry.required("direction", DIRECTION, direction)?;
        mappings.push(Mapping {
            tag,
            topic,
            direction,
        });
    }
    Ok(Some(mappings))
}
