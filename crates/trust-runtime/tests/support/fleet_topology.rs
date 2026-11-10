//! What a runtime child reports about its own web interface in its `fleet.topology` answer
//! (specification 11, fleet topology), for the tests that start runtime children. Included by
//! `runtime_process.rs`.

use serde_json::Value;

/// The address of the runtime's own web endpoint in a `fleet.topology` answer, once its listener is
/// bound (health `connected`): with a configured port 0, the port the operating system chose.
pub fn bound_web_address(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if map.get("protocol").and_then(Value::as_str) == Some("web")
                && map.get("source").and_then(Value::as_str) == Some("self")
                && map.get("health").and_then(Value::as_str) == Some("connected")
            {
                return map
                    .get("address")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            map.values().find_map(bound_web_address)
        }
        Value::Array(items) => items.iter().find_map(bound_web_address),
        _ => None,
    }
}
