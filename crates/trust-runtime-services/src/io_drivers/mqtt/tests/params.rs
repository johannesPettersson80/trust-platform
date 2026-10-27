//! The `params` table of the `mqtt` driver (specification 32 §2 to §5), with the keys, defaults
//! and refusals of trust-runtime's driver.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;

use crate::io_drivers::mqtt::{
    BrokerEndpoint, ClientAuth, Credentials, Mapping, MappingDirection, MqttParams, MqttTls,
};
use crate::io_drivers::{OnError, ParamsError};

/// The refusal of `key` for `reason`.
fn refused(key: &str, reason: &str) -> Result<MqttParams, ParamsError> {
    Err(ParamsError {
        key: key.into(),
        reason: reason.into(),
    })
}

/// The parameters of the broker `127.0.0.1:1883` with `fields`, more JSON members.
fn local_with(fields: &str) -> Result<MqttParams, ParamsError> {
    MqttParams::from_json(&format!(r#"{{"broker":"127.0.0.1:1883",{fields}}}"#))
}

/// The parameters of the remote broker `b.example:8883` over TLS with its authority at
/// `/ca.pem`, and `fields`, more JSON members that each start with a comma.
fn tls_with(fields: &str) -> Result<MqttParams, ParamsError> {
    MqttParams::from_json(&format!(
        r#"{{"broker":"b.example:8883","tls":true,"tls_ca_path":"/ca.pem"{fields}}}"#
    ))
}

/// The broker of `broker`, allowed to be remote without TLS.
fn endpoint(broker: &str) -> Result<BrokerEndpoint, ParamsError> {
    MqttParams::from_json(&format!(
        r#"{{"allow_insecure_remote":true,"broker":"{broker}"}}"#
    ))
    .map(|params| params.broker)
}

/// A local broker takes trust-runtime's defaults: topics `trust/io/in` and `trust/io/out`
/// (specification 32 §4), a reconnect backoff of 500 ms, a keep alive of 5 s, `on_error =
/// "fault"`, no credentials and no TLS, both raw directions on. Catches a default that differs
/// from the computer's, which would change a project that moves to a chip.
#[test]
fn a_local_broker_takes_the_defaults() {
    let params = MqttParams::from_json(r#"{"broker":"mqtt://127.0.0.1:1883"}"#);
    assert_eq!(
        params,
        Ok(MqttParams {
            broker: BrokerEndpoint {
                host: "127.0.0.1".into(),
                port: 1883,
            },
            allow_insecure_remote: false,
            client_id: None,
            topic_in: "trust/io/in".into(),
            topic_out: "trust/io/out".into(),
            credentials: None,
            reconnect_ms: 500,
            keep_alive_s: 5,
            on_error: OnError::Fault,
            tls: None,
            mappings: None,
            input_points: vec![],
            output_points: vec![],
            sparkplug: None,
        })
    );
    let params = params.expect("a valid table");
    assert!(params.input_enabled() && params.output_enabled());
    assert_eq!(params.subscribe_topics(), vec![SmolStr::new("trust/io/in")]);
}

/// Every key reads into its value: the scheme `mqtts://` turns TLS on, paths and topics are
/// trimmed, `on_error` is read trimmed and in any case with `warning` for `warn`, and a given 0 for
/// `reconnect_ms` and `keep_alive_s` is read as 1, as the computer reads them. Catches a key read
/// into the wrong field, an untrimmed path, and a busy reconnect loop from a backoff of 0.
#[test]
fn a_full_table_reads_into_its_values() {
    let params = MqttParams::from_json(concat!(
        r#"{"broker":" mqtts://broker.example:8883 ","client_id":"line-7","keep_alive_s":0,"#,
        r#""on_error":" Warning ","password":"s3cret","reconnect_ms":0,"tls_alpn":[" mqtt "],"#,
        r#""tls_ca_path":" /etc/ca.pem ","tls_client_cert_path":"/etc/c.pem","#,
        r#""tls_client_key_path":"/etc/k.pem","topic_in":" in/x ","topic_out":"out/x","#,
        r#""username":"plc"}"#
    ));
    assert_eq!(
        params,
        Ok(MqttParams {
            broker: BrokerEndpoint {
                host: "broker.example".into(),
                port: 8883,
            },
            allow_insecure_remote: false,
            client_id: Some("line-7".into()),
            topic_in: "in/x".into(),
            topic_out: "out/x".into(),
            credentials: Some(Credentials {
                username: "plc".into(),
                password: "s3cret".into(),
            }),
            reconnect_ms: 1,
            keep_alive_s: 1,
            on_error: OnError::Warn,
            tls: Some(MqttTls {
                ca_path: "/etc/ca.pem".into(),
                client_auth: Some(ClientAuth {
                    cert_path: "/etc/c.pem".into(),
                    key_path: "/etc/k.pem".into(),
                }),
                alpn: Some(vec!["mqtt".into()]),
            }),
            mappings: None,
            input_points: vec![],
            output_points: vec![],
            sparkplug: None,
        })
    );
}

/// The broker is `host:port` after `mqtt://`, `tcp://`, `mqtts://` or `ssl://` or no scheme, an
/// IPv6 host in brackets (specification 32 §3). Catches a scheme or a bracket left in the host, a
/// missing port taken, port 0 taken, an unknown scheme read as part of a host name, and a
/// password in the broker's user information shown in a refusal.
#[test]
fn the_broker_is_host_and_port_after_a_known_scheme() {
    let host = |name: &str, port| {
        Ok(BrokerEndpoint {
            host: name.into(),
            port,
        })
    };
    assert_eq!(endpoint("tcp://10.0.0.5:1883"), host("10.0.0.5", 1883));
    assert_eq!(endpoint("10.0.0.5:1884"), host("10.0.0.5", 1884));
    assert_eq!(endpoint("[fe80::1]:1883"), host("fe80::1", 1883));
    assert_eq!(
        endpoint("mqtt:// b.example : 1883 "),
        host("b.example", 1883)
    );
    let what = "the broker's host:port, such as \"mqtt://127.0.0.1:1883\"";
    assert_eq!(
        MqttParams::from_json("{}"),
        refused("broker", &format!("is required, {what}"))
    );
    assert_eq!(
        MqttParams::from_json(r#"{"broker":1883}"#),
        refused("broker", &format!("must be a string, {what}, not 1883"))
    );
    for (broker, reason) in [
        (
            "mqtt://127.0.0.1",
            r#"must be host:port, not "mqtt://127.0.0.1""#,
        ),
        ("mqtt://:1883", r#"has an empty host: "mqtt://:1883""#),
        ("[]:1883", r#"has an empty host: "[]:1883""#),
        (
            "127.0.0.1:0",
            r#"has port 0 in "127.0.0.1:0"; a port is from 1 to 65535"#,
        ),
        (
            "127.0.0.1:65536",
            r#"has an invalid port in "127.0.0.1:65536": number too large to fit in target type"#,
        ),
        (
            "ws://127.0.0.1:1883",
            concat!(
                "must use mqtt://, tcp://, mqtts:// or ssl://, or no scheme, ",
                r#"not "ws://127.0.0.1:1883""#
            ),
        ),
        (
            "mqtt://plc:s3cret@127.0.0.1:1883",
            "must not carry user information: give username and password instead",
        ),
    ] {
        assert_eq!(
            MqttParams::from_json(&format!(r#"{{"broker":"{broker}"}}"#)),
            refused("broker", reason),
            "{broker}"
        );
    }
}

/// A plaintext broker is taken only when it is local, `localhost` in any case, `127.0.0.1` or
/// `::1`, or with `allow_insecure_remote = true` (specification 32 §3). Catches a remote broker
/// reached in plaintext without the allowance.
#[test]
fn a_remote_broker_needs_tls_or_the_insecure_allowance() {
    let remote = |host: &str| {
        format!(
            "must be true for the remote broker \"{host}\" without TLS; only localhost, \
             127.0.0.1 and ::1 are local"
        )
    };
    assert_eq!(
        MqttParams::from_json(r#"{"broker":"10.10.0.9:1883"}"#),
        refused("allow_insecure_remote", &remote("10.10.0.9"))
    );
    assert_eq!(
        MqttParams::from_json(r#"{"broker":"127.0.0.2:1883"}"#),
        refused("allow_insecure_remote", &remote("127.0.0.2"))
    );
    for local in [
        r#"{"broker":"LocalHost:1883"}"#,
        r#"{"broker":"[::1]:1883"}"#,
        r#"{"allow_insecure_remote":true,"broker":"10.10.0.9:1883"}"#,
    ] {
        assert!(MqttParams::from_json(local).is_ok(), "{local}");
    }
    assert_eq!(
        local_with(r#""allow_insecure_remote":"yes""#),
        refused(
            "allow_insecure_remote",
            r#"must be true or false, not "yes""#
        )
    );
}

/// TLS is on with `tls = true` or a TLS scheme, which `tls = false` contradicts; with it off a
/// TLS path or a non-empty ALPN list is refused and an empty list has no effect; with it on the
/// authority's path is required and the client's certificate and key come together
/// (specification 32 §3). Catches a TLS setting silently ignored, TLS without trust roots, and
/// half a mutual-TLS pair.
#[test]
fn tls_settings_follow_their_rules() {
    assert_eq!(
        tls_with("").map(|params| params.tls),
        Ok(Some(MqttTls {
            ca_path: "/ca.pem".into(),
            client_auth: None,
            alpn: None,
        }))
    );
    assert_eq!(
        MqttParams::from_json(r#"{"broker":"mqtts://b.example:8883"}"#),
        refused(
            "tls_ca_path",
            "is required with TLS, the path of the PEM file of the broker's certificate authority"
        )
    );
    assert_eq!(
        MqttParams::from_json(r#"{"broker":"mqtts://b.example:8883","tls":false}"#),
        refused(
            "tls",
            "is false, but the broker's mqtts:// or ssl:// scheme is TLS"
        )
    );
    let needs_tls = "needs TLS: set tls = true or use an mqtts:// or ssl:// broker";
    assert_eq!(
        local_with(r#""tls_ca_path":"/ca.pem""#),
        refused("tls_ca_path", needs_tls)
    );
    assert_eq!(
        local_with(r#""tls_alpn":["mqtt"]"#),
        refused("tls_alpn", needs_tls)
    );
    assert_eq!(
        local_with(r#""tls":false,"tls_alpn":[]"#).map(|params| params.tls),
        Ok(None)
    );
    let both = "mutual TLS takes both";
    assert_eq!(
        tls_with(r#","tls_client_cert_path":"/c.pem""#),
        refused(
            "tls_client_key_path",
            &format!("is required with tls_client_cert_path: {both}")
        )
    );
    assert_eq!(
        tls_with(r#","tls_client_key_path":"/k.pem""#),
        refused(
            "tls_client_cert_path",
            &format!("is required with tls_client_key_path: {both}")
        )
    );
    assert_eq!(
        tls_with(r#","tls_alpn":["mqtt"," "]"#),
        refused("tls_alpn[1]", "must not be empty")
    );
    assert_eq!(
        tls_with(r#","tls_alpn":"mqtt""#),
        refused("tls_alpn", r#"must be an array of strings, not "mqtt""#)
    );
    assert_eq!(
        MqttParams::from_json(r#"{"broker":"b.example:8883","tls":true,"tls_ca_path":"  "}"#),
        refused("tls_ca_path", "must not be empty")
    );
}

/// `username` and `password` come together, the user name is not empty, an empty password is
/// taken, and neither value appears in a refusal or in the `Debug` of the parameters
/// (specification 32 §3). Catches half a credential pair taken, and a secret written to a log.
#[test]
fn credentials_come_together_and_never_show() {
    assert_eq!(
        local_with(r#""username":"plc""#),
        refused(
            "password",
            "is required with username: the two are given together"
        )
    );
    assert_eq!(
        local_with(r#""password":"s3cret""#),
        refused(
            "username",
            "is required with password: the two are given together"
        )
    );
    assert_eq!(
        local_with(r#""password":"s3cret","username":"""#),
        refused(
            "username",
            "must be a non-empty string of at most 65535 bytes without U+0000"
        )
    );
    assert_eq!(
        local_with(r#""password":123456,"username":"plc""#),
        refused("password", "must be a string of at most 65535 bytes")
    );
    assert_eq!(
        local_with(r#""password":"","username":"plc""#).map(|params| params.credentials),
        Ok(Some(Credentials {
            username: "plc".into(),
            password: SmolStr::default(),
        }))
    );
    let params = local_with(r#""password":"hunter2","username":"plc""#).expect("credentials");
    let shown = format!("{params:?}");
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(shown.contains(r#"password: "<redacted>""#), "{shown}");
}

/// `keep_alive_s` is 0 to 65,535 and `reconnect_ms` a whole number of milliseconds, and
/// `on_error` is `fault`, `warn` (or `warning`) or `ignore` in any case. Catches a keep alive that
/// does not fit CONNECT's two bytes, a negative backoff, and a misspelt policy taken as `fault`.
#[test]
fn numbers_and_the_policy_are_checked() {
    assert_eq!(
        local_with(r#""keep_alive_s":65535"#).map(|params| params.keep_alive_s),
        Ok(65_535)
    );
    assert_eq!(
        local_with(r#""keep_alive_s":65536"#),
        refused(
            "keep_alive_s",
            "must be an integer from 0 to 65535, not 65536"
        )
    );
    for given in ["-1", "2.5", "\"500\""] {
        assert_eq!(
            local_with(&format!(r#""reconnect_ms":{given}"#)),
            refused(
                "reconnect_ms",
                &format!("must be an integer from 0 to 9223372036854775807, not {given}")
            )
        );
    }
    for (word, policy) in [
        ("fault", OnError::Fault),
        ("WARN", OnError::Warn),
        (" warning ", OnError::Warn),
        ("Ignore", OnError::Ignore),
    ] {
        assert_eq!(
            local_with(&format!(r#""on_error":"{word}""#)).map(|params| params.on_error),
            Ok(policy),
            "{word}"
        );
    }
    assert_eq!(
        local_with(r#""on_error":"halt""#),
        refused(
            "on_error",
            r#"must be "fault", "warn" or "ignore", not "halt""#
        )
    );
}

/// An unknown key is refused in the table, in a point, in a mapping and in the `sparkplug` table
/// (specification 32 §2: unknown fields are errors, not ignored). Catches a misspelt key that
/// would be ignored, such as `metric_name` on an input point, which only output metrics have.
#[test]
fn an_unknown_key_is_refused_everywhere() {
    assert_eq!(
        local_with(r#""qos":1"#),
        refused(
            "qos",
            "unknown key; the mqtt driver takes allow_insecure_remote, broker, client_id, \
             input_points, keep_alive_s, mappings, on_error, output_points, password, \
             reconnect_ms, sparkplug, tls, tls_alpn, tls_ca_path, tls_client_cert_path, \
             tls_client_key_path, topic_in, topic_out and username"
        )
    );
    assert_eq!(
        local_with(
            r#""input_points":[{"data_type":"u16","image_offset":0,"metric_name":"m","topic":"a"}]"#
        ),
        refused(
            "input_points[0].metric_name",
            "unknown key; an input point takes data_type, image_bit, image_offset, offset, \
             payload_format, scale and topic"
        )
    );
    assert_eq!(
        local_with(r#""mappings":[{"direction":"read","qos":0,"tag":"Main.X","topic":"a"}]"#),
        refused(
            "mappings[0].qos",
            "unknown key; a mapping takes direction, tag and topic"
        )
    );
    assert_eq!(
        local_with(r#""sparkplug":{"group":"g"}"#),
        refused(
            "sparkplug.group",
            "unknown key; the sparkplug table takes birth_death_seq, edge_node_id, enabled, \
             group_id, namespace and spec_version"
        )
    );
}

/// The presence of `mappings` selects mapped mode (specification 32 §4): an empty list turns both
/// raw directions off, a lowered write mapping subscribes to nothing, a lowered read mapping
/// subscribes to its point's topic, and without `mappings` typed outputs leave the raw input on.
/// Catches the implicit `trust/io/in` subscription of a write-only mapping, which the
/// specification calls a configuration-mode defect.
#[test]
fn mappings_select_mapped_mode() {
    let empty = local_with(r#""mappings":[]"#).expect("an empty mapping list");
    assert_eq!(empty.mappings, Some(vec![]));
    assert!(!empty.input_enabled() && !empty.output_enabled());
    assert_eq!(empty.subscribe_topics(), Vec::<SmolStr>::new());
    let write_only = local_with(concat!(
        r#""mappings":[{"direction":"write","tag":"MainInstance.Green","#,
        r#""topic":"traffic/north/green"}],"output_points":[{"data_type":"bool","#,
        r#""image_bit":0,"image_offset":0,"topic":"traffic/north/green"}]"#
    ))
    .expect("a lowered write mapping");
    assert_eq!(
        write_only.mappings,
        Some(vec![Mapping {
            tag: "MainInstance.Green".into(),
            topic: "traffic/north/green".into(),
            direction: MappingDirection::Write,
        }])
    );
    assert!(!write_only.input_enabled() && write_only.output_enabled());
    assert_eq!(write_only.subscribe_topics(), Vec::<SmolStr>::new());
    let read_only = local_with(concat!(
        r#""input_points":[{"data_type":"u16","image_offset":0,"topic":"line/level"}],"#,
        r#""mappings":[{"direction":"read","tag":"MainInstance.Level","topic":"line/level"}]"#
    ))
    .expect("a lowered read mapping");
    assert!(read_only.input_enabled() && !read_only.output_enabled());
    assert_eq!(
        read_only.subscribe_topics(),
        vec![SmolStr::new("line/level")]
    );
    let outputs = local_with(
        r#""output_points":[{"data_type":"u16","image_offset":0,"topic":"line/speed"}]"#,
    )
    .expect("typed outputs");
    assert!(outputs.input_enabled());
    assert_eq!(
        outputs.subscribe_topics(),
        vec![SmolStr::new("trust/io/in")]
    );
}

/// A mapping is exactly `tag`, `topic` and `direction` (specification 32 §5), and a raw topic is
/// an exact topic like a point's. Catches a mapping without a tag, a direction in the wrong case,
/// and a wildcard `topic_in` whose messages could never match.
#[test]
fn mappings_and_raw_topics_are_checked() {
    assert_eq!(
        local_with(r#""mappings":[{"direction":"read","topic":"a"}]"#),
        refused(
            "mappings[0].tag",
            concat!(
                "is required, the fully qualified name of a program variable, ",
                r#"such as "MainInstance.Green""#
            )
        )
    );
    assert_eq!(
        local_with(r#""mappings":[{"direction":"READ","tag":"Main.X","topic":"a"}]"#),
        refused(
            "mappings[0].direction",
            r#"must be "read" or "write", not "READ""#
        )
    );
    assert_eq!(
        local_with(r#""mappings":{}"#),
        refused("mappings", "must be an array of tables, not {}")
    );
    assert_eq!(
        local_with(r#""topic_in":"line/#""#),
        refused(
            "topic_in",
            "must be an exact MQTT topic: not empty, at most 65535 bytes, without the \
             wildcards + and # and without control characters, not \"line/#\""
        )
    );
    assert_eq!(
        local_with(r#""client_id":"""#).map(|params| params.client_id),
        Ok(Some(SmolStr::default()))
    );
    assert_eq!(
        local_with(r#""client_id":7"#),
        refused(
            "client_id",
            "must be a string of at most 65535 bytes without U+0000, not 7"
        )
    );
}

/// The authentication context under which a redacted edit keeps a stored password is the broker,
/// the TLS settings, the user name, the client identifier and `allow_insecure_remote`
/// (specification 32 §3); a new password or new topics do not change it. Catches a stored
/// password reused for another broker, user or client.
#[test]
fn the_credential_context_is_broker_tls_user_client_and_allowance() {
    let stored = local_with(r#""password":"one","username":"plc""#).expect("stored");
    let same = |json: &str| {
        stored.same_credential_context(&MqttParams::from_json(json).expect("requested"))
    };
    assert!(same(
        r#"{"broker":"127.0.0.1:1883","password":"two","topic_out":"x/y","username":"plc"}"#
    ));
    for changed in [
        r#"{"broker":"127.0.0.1:1884","password":"one","username":"plc"}"#,
        r#"{"broker":"127.0.0.1:1883","password":"one","username":"ops"}"#,
        r#"{"broker":"127.0.0.1:1883","client_id":"x","password":"one","username":"plc"}"#,
        concat!(
            r#"{"allow_insecure_remote":true,"broker":"127.0.0.1:1883","#,
            r#""password":"one","username":"plc"}"#
        ),
    ] {
        assert!(!same(changed), "{changed}");
    }
}
