//! The fields of an `mqtt` connection's form (specification 32 §2): the driver's parameters with its
//! own bounds, so that the form takes every setting the driver runs with.

use serde_json::json;
use trust_runtime_services::io_drivers::mqtt::MqttParams;

use super::{
    advanced, endpoint_field, enum_field, field, number_field, optional, secret, toml_integer,
    CommFieldSchema,
};

pub(in crate::control::comm_handlers::schema) fn mqtt_fields() -> Vec<CommFieldSchema> {
    vec![
        endpoint_field(
            "broker",
            "Broker",
            "127.0.0.1:1883",
            "MQTT broker host and port.",
        ),
        optional(field(
            "client_id",
            "Client ID",
            "string",
            json!(""),
            false,
            "Optional MQTT client id. Empty lets the runtime choose one.",
        )),
        field(
            "topic_in",
            "Input topic",
            "string",
            json!("trust/io/in"),
            true,
            "Topic used for incoming process values.",
        ),
        field(
            "topic_out",
            "Output topic",
            "string",
            json!("trust/io/out"),
            true,
            "Topic used for outgoing process values.",
        ),
        optional(field(
            "username",
            "Username",
            "string",
            json!(""),
            false,
            "Broker username. Use together with password.",
        )),
        optional(secret(field(
            "password",
            "Password",
            "secret",
            serde_json::Value::Null,
            false,
            "Broker password. Existing saved passwords are not shown here.",
        ))),
        field(
            "tls",
            "TLS",
            "boolean",
            json!(false),
            false,
            "Use TLS for the MQTT connection.",
        ),
        field(
            "allow_insecure_remote",
            "Allow insecure remote broker",
            "boolean",
            json!(false),
            false,
            "Required when using a non-TLS broker that is not local.",
        ),
        number_field(
            "reconnect_ms",
            "Reconnect delay",
            500,
            toml_integer(MqttParams::RECONNECT_MS_RANGE.0),
            toml_integer(MqttParams::RECONNECT_MS_RANGE.1),
            "Delay between reconnect attempts in milliseconds; 0 is read as 1.",
        ),
        number_field(
            "keep_alive_s",
            "Keepalive",
            5,
            i64::from(MqttParams::KEEP_ALIVE_S_RANGE.0),
            i64::from(MqttParams::KEEP_ALIVE_S_RANGE.1),
            "MQTT keepalive in seconds; 0 is read as 1.",
        ),
        enum_field(
            "on_error",
            "On error",
            "fault",
            vec!["fault", "warn", "ignore"],
            "Runtime behavior when the MQTT broker is unreachable, a message does not convert, or an input is older than Stale after.",
        ),
        advanced(optional(field(
            "tls_ca_path",
            "TLS CA path",
            "path",
            json!(""),
            false,
            "CA certificate path when TLS is enabled.",
        ))),
        advanced(optional(field(
            "tls_client_cert_path",
            "Client cert path",
            "path",
            json!(""),
            false,
            "Client certificate path for mTLS.",
        ))),
        advanced(optional(field(
            "tls_client_key_path",
            "Client key path",
            "path",
            json!(""),
            false,
            "Client key path for mTLS.",
        ))),
        advanced(optional(field(
            "tls_alpn",
            "TLS ALPN",
            "json_array",
            json!([]),
            false,
            "Optional ALPN protocol list.",
        ))),
        advanced(number_field(
            "stale_after_ms",
            "Stale after (ms)",
            0,
            toml_integer(MqttParams::STALE_AFTER_MS_RANGE.0),
            toml_integer(MqttParams::STALE_AFTER_MS_RANGE.1),
            "0 keeps an input's last value while the broker is connected; a larger value makes an input whose last message is older stale, and it follows On error.",
        )),
    ]
}
