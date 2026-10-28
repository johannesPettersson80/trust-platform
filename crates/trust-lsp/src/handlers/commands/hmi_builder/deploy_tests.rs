//! The deployment's web server (specification 39, §16.2 item 8).

use serde_json::{json, Value};

use super::{answer, web_address};

fn topology(listen: &str, tls: bool) -> Value {
    json!({
        "schema_version": 1,
        "hosts": [{
            "host_id": "h",
            "runtimes": [{ "runtime_id": "r", "endpoints": [
                { "id": "control", "protocol": "control", "source": "self", "params": { "listen": "0.0.0.0:9000" } },
                { "id": "web", "kind": "service", "protocol": "web", "source": "self",
                  "params": { "listen": listen, "tls": tls, "auth": "token" } }
            ] }],
            "containers": []
        }]
    })
}

#[test]
fn the_address_is_the_listen_port_on_the_control_endpoints_host() {
    // A runtime listening on every interface (0.0.0.0, ::) is reached on the host the editor
    // already talks to; sending to 0.0.0.0 would reach this computer, not the runtime.
    assert_eq!(
        web_address("tcp://192.168.1.20:9000", "0.0.0.0:8080", false).as_deref(),
        Some("http://192.168.1.20:8080")
    );
    assert_eq!(
        web_address("unix:///run/trust/control.sock", "127.0.0.1:8080", true).as_deref(),
        Some("https://127.0.0.1:8080"),
        "a local socket is this machine; TLS gives https"
    );
    assert_eq!(
        web_address("tcp://plc.local:9000", "10.0.0.5:8443", true).as_deref(),
        Some("https://10.0.0.5:8443"),
        "a listen address of its own wins"
    );
    assert_eq!(
        web_address("tcp://[::1]:9000", "[::]:8080", false).as_deref(),
        Some("http://[::1]:8080"),
        "an IPv6 host stays in brackets"
    );
    assert_eq!(
        web_address("npipe:trust", " [::]:80 ", false).as_deref(),
        Some("http://127.0.0.1:80"),
        "a named pipe is this machine; the listen address is trimmed"
    );
    assert_eq!(
        web_address("tcp://plc.local:9000", "not an address", false),
        None
    );
    assert_eq!(
        web_address("tcp://plc.local:9000", "::1:8080", false),
        None,
        "an IPv6 address needs brackets"
    );
    assert_eq!(
        web_address("tcp://plc.local:9000", "plc:http", false),
        None,
        "the port is a number"
    );
}

#[test]
fn the_runtime_reports_its_own_web_server_in_its_topology() {
    assert_eq!(
        answer(json!({ "controlEndpoint": "tcp://192.168.1.20:9000", "topology": topology("0.0.0.0:8080", false) }))
            .unwrap(),
        json!({ "ok": true, "url": "http://192.168.1.20:8080" })
    );
    // A web server another runtime reports (source "peer") is not this runtime's own.
    let mut peer = topology("10.0.0.9:8080", false);
    peer["hosts"][0]["runtimes"][0]["endpoints"][1]["source"] = json!("peer");
    assert_eq!(
        answer(json!({ "controlEndpoint": "tcp://192.168.1.20:9000", "topology": peer })).unwrap(),
        json!({ "ok": true, "url": null })
    );
    // A runtime in a container on the host.
    let contained = json!({ "hosts": [{ "runtimes": [], "containers": [{ "runtimes": [{ "endpoints": [
        { "protocol": "web", "source": "self", "params": { "listen": "0.0.0.0:8443", "tls": true } }
    ] }] }] }] });
    assert_eq!(
        answer(json!({ "controlEndpoint": "tcp://box:9000", "topology": contained })).unwrap(),
        json!({ "ok": true, "url": "https://box:8443" })
    );
}
