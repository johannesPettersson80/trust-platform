use super::*;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread;

use crate::protocol::AdsStateEventBody;

#[test]
fn local_st_ads_state_returns_and_emits_the_same_runtime_snapshot() {
    let mut runtime = Runtime::new();
    runtime.execute_cycle().expect("advance runtime scan");
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));

    let outcome = adapter.dispatch_request(ads_state_request(1));

    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<AdsStateEventBody> =
        serde_json::from_value(outcome.responses[0].clone()).expect("ADS state response");
    let event: Event<AdsStateEventBody> =
        serde_json::from_value(outcome.events[0].clone()).expect("ADS state event");
    assert!(response.success);
    assert_eq!(event.event, "stAdsState");
    let response_body = response.body.expect("ADS response body");
    let event_body = event.body.expect("ADS event body");
    assert_eq!(response_body, event_body);
    assert_eq!(response_body.schema_version, 1);
    assert_eq!(response_body.scan, 1);
    assert!(response_body.entries.is_empty());
    assert_eq!(outcome.events[0]["body"]["schemaVersion"], 1);
}

#[test]
fn local_st_ads_state_uses_the_last_scan_snapshot_while_runtime_is_busy() {
    let mut adapter = DebugAdapter::new(DebugSession::new(Runtime::new()));
    let first = adapter.dispatch_request(ads_state_request(10));
    let first: Response<AdsStateEventBody> =
        serde_json::from_value(first.responses[0].clone()).expect("first ADS state");
    let expected = first.body.expect("first ADS body");

    let runtime = adapter.session().runtime_handle();
    let _runtime_guard = runtime.lock().expect("hold runtime during ADS request");
    let cached = adapter.dispatch_request(ads_state_request(11));
    let cached: Response<AdsStateEventBody> =
        serde_json::from_value(cached.responses[0].clone()).expect("cached ADS state");

    assert!(cached.success);
    assert_eq!(cached.body.as_ref(), Some(&expected));
}

#[test]
fn remote_st_ads_state_uses_ads_live_values_without_schema_translation() {
    let (addr, server) = spawn_remote_ads_live_values_server();
    let mut adapter = DebugAdapter::new(DebugSession::new(Runtime::new()));
    adapter.remote_session = Some(
        super::super::remote::RemoteSession::connect(
            super::super::remote::RemoteEndpoint::Tcp(addr),
            Some("viewer-token".to_string()),
        )
        .expect("remote session"),
    );

    let outcome = adapter.dispatch_request(ads_state_request(2));

    let response: Response<AdsStateEventBody> =
        serde_json::from_value(outcome.responses[0].clone()).expect("ADS state response");
    let event: Event<AdsStateEventBody> =
        serde_json::from_value(outcome.events[0].clone()).expect("ADS state event");
    assert!(response.success, "remote ADS state: {:?}", response.message);
    let body = response.body.expect("response body");
    assert_eq!(event.body.as_ref(), Some(&body));
    assert_eq!(body.scan, 44);
    assert_eq!(body.entries.len(), 1);
    let entry = &body.entries[0];
    assert_eq!(entry.connection, "line1");
    assert_eq!(entry.name, "line1_temp");
    assert_eq!(entry.remote_symbol, "MAIN.Temperature");
    assert_eq!(entry.value, "42.5");
    assert_eq!(entry.value_type, "REAL");
    assert_eq!(entry.access, "read");
    assert_eq!(
        serde_json::to_value(&entry.quality).expect("serialize quality")["state"],
        "stale"
    );
    assert_eq!(entry.quality.last_update_ms, Some(40));
    assert_eq!(entry.quality.detail.as_deref(), Some("route retry"));

    drop(adapter);
    server.join().expect("remote ADS server");
}

fn ads_state_request(seq: u32) -> Request<serde_json::Value> {
    Request {
        seq,
        message_type: MessageType::Request,
        command: "stAdsState".to_string(),
        arguments: None,
    }
}

fn spawn_remote_ads_live_values_server() -> (SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind remote ADS server");
    let addr = listener.local_addr().expect("remote ADS address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("remote ADS client");
        let mut reader = BufReader::new(stream.try_clone().expect("clone ADS stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read ADS request");
        let request: serde_json::Value = serde_json::from_str(&line).expect("ADS request JSON");
        assert_eq!(request["type"], "ads.live_values");
        assert_eq!(request["auth"], "viewer-token");
        let response = serde_json::json!({
            "id": request["id"],
            "ok": true,
            "result": {
                "schemaVersion": 1,
                "scan": 44,
                "entries": [{
                    "connection": "line1",
                    "name": "line1_temp",
                    "remoteSymbol": "MAIN.Temperature",
                    "value": "42.5",
                    "valueType": "REAL",
                    "access": "read",
                    "quality": {
                        "state": "stale",
                        "lastUpdateMs": 40,
                        "detail": "route retry"
                    }
                }]
            }
        });
        writeln!(stream, "{}", response).expect("write ADS response");
        stream.flush().expect("flush ADS response");
    });
    (addr, server)
}
