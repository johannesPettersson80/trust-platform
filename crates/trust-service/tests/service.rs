//! The SDK against a scripted runtime (specification 48 RU-62, RU-63; specification 11 §6.8.1): the
//! requests a service sends, with its service token, and how it reads the replies. The runtime
//! serves every connection on a thread of its own, as the heartbeat has its own.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime_client::ControlError;
use trust_service::{Quality, Service, ServiceError, ServiceSettings, Variable, Writable};

/// The requests a scripted runtime received, in order.
type Requests = Arc<Mutex<Vec<Value>>>;

/// A runtime on a local port that answers each request with `answer(request)`, a response without
/// its `id`, which it takes from the request.
fn runtime(answer: impl Fn(&Value) -> Value + Send + Sync + 'static) -> (String, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("the port"));
    let requests: Requests = Arc::new(Mutex::new(Vec::new()));
    let answer = Arc::new(answer);
    let received = Arc::clone(&requests);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let received = Arc::clone(&received);
            let answer = Arc::clone(&answer);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 {
                    let request: Value = serde_json::from_str(line.trim()).expect("a request");
                    line.clear();
                    let mut response = answer(&request);
                    response["id"] = request["id"].clone();
                    received.lock().expect("the requests").push(request);
                    if stream
                        .write_all(format!("{response}\n").as_bytes())
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    (endpoint, requests)
}

/// The settings of the service recipe at `endpoint`, with a heartbeat every `heartbeat`.
fn settings(endpoint: &str, heartbeat: Duration) -> ServiceSettings {
    ServiceSettings {
        name: "recipe".into(),
        endpoint: endpoint.into(),
        token: "service-token".into(),
        heartbeat,
    }
}

/// What a runtime of specification 11 §6.8.1 answers each of the three requests.
fn answers(request: &Value) -> Value {
    match request["type"].as_str() {
        Some("variables.read") => json!({"ok": true, "result": {
            "scan": 7, "time_ns": 70_000_000, "state": "running", "quality": "good",
            "values": [{"path": "P1.level", "type": "REAL", "value": 1.5}],
        }}),
        Some("variables.write") => json!({"ok": true, "result": {"service": "recipe", "count": 1}}),
        Some("service.heartbeat") => {
            json!({"ok": true, "result": {"service": "recipe", "value": 42}})
        }
        _ => json!({"ok": false, "error": "unsupported request"}),
    }
}

/// The requests of `kind` among `requests`.
fn of_kind(requests: &Requests, kind: &str) -> Vec<Value> {
    requests
        .lock()
        .expect("the requests")
        .iter()
        .filter(|request| request["type"] == json!(kind))
        .cloned()
        .collect()
}

#[test]
fn a_service_reads_writes_and_beats_with_its_token() {
    // Each request as specification 11 §6.8.1 gives it, with the service token as `auth`: a read
    // of the paths, a write of path and value, a heartbeat naming its period of 1 s as `period_ms`
    // 1000 (RU-62). The read's reply is
    // a snapshot whose REAL reads as an f64; the write gives the number of values; the heartbeat
    // the value the next cycle writes. Catches a request sent without the token, a write of
    // another shape, and a reply read from the wrong field.
    let (endpoint, requests) = runtime(answers);
    let service =
        Service::connect_with(settings(&endpoint, Duration::from_secs(1))).expect("a connection");

    let read = service.read(&["P1.level"]).expect("a read");
    assert_eq!((read.scan, read.quality), (7, Quality::Good));
    assert_eq!(read.get::<f64>("P1.level"), Ok(1.5));
    assert_eq!(service.write(&[("P1.target", json!(21.5))]), Ok(1));
    assert_eq!(service.heartbeat(), Ok(42));

    let sent: Vec<Value> = requests
        .lock()
        .expect("the requests")
        .iter()
        .map(|request| json!({"type": request["type"], "auth": request["auth"], "params": request["params"]}))
        .collect();
    assert_eq!(
        sent,
        vec![
            json!({"type": "variables.read", "auth": "service-token", "params": {"paths": ["P1.level"]}}),
            json!({"type": "variables.write", "auth": "service-token",
                   "params": {"writes": [{"path": "P1.target", "value": 21.5}]}}),
            json!({"type": "service.heartbeat", "auth": "service-token",
                   "params": {"period_ms": 1000}}),
        ]
    );
}

#[test]
fn a_refusal_is_the_runtimes_text() {
    // A write the runtime refuses is an error with its text and code, and nothing else. Catches a
    // refusal turned into a count of zero, which a service could not tell from a success.
    let (endpoint, _) = runtime(
        |_| json!({"ok": false, "error": "service recipe may not write P1.Counter", "error_code": "invalid_request"}),
    );
    let service =
        Service::connect_with(settings(&endpoint, Duration::from_secs(1))).expect("a connection");
    let refused = service
        .write(&[("P1.Counter", json!(0))])
        .expect_err("refused");
    assert_eq!(
        refused.to_string(),
        "service recipe may not write P1.Counter"
    );
    assert!(matches!(
        refused,
        ServiceError::Control(ControlError::Refused { .. })
    ));
}

#[test]
fn a_heartbeat_names_its_period() {
    // RU-62 and specification 11 §6.8.1: each heartbeat names the service's period as `period_ms`,
    // by which the runtime tells how old its last heartbeat may be (RU-64); a period below a
    // millisecond names 1 ms, the smallest the runtime takes. Catches a heartbeat whose runtime
    // must guess its period, and a sub-millisecond period sent as 0, which the runtime refuses.
    for (period, named) in [
        (Duration::from_millis(40), 40),
        (Duration::from_micros(500), 1),
    ] {
        let (endpoint, requests) = runtime(answers);
        let service = Service::connect_with(settings(&endpoint, period)).expect("a connection");
        assert_eq!(service.heartbeat().expect("the heartbeat"), 42);
        let beats = of_kind(&requests, "service.heartbeat");
        assert_eq!(beats.len(), 1);
        assert_eq!(beats[0]["params"], json!({ "period_ms": named }));
    }
}

#[test]
fn the_heartbeat_thread_beats_at_its_period_until_it_is_dropped() {
    // A heartbeat every 40 ms over at least 300 ms: the first at once and then one per period, so
    // at least 2 and at most one more than the periods in the time that passed, which a slow read
    // on a busy computer lengthens (macOS CI, #628: 11 beats); none after the heartbeat is
    // dropped, over the next five periods. The heartbeats come on a connection of their own: the
    // service's read between them is answered too. Catches a heartbeat that never starts, one that
    // floods the runtime, and one that outlives its service.
    let (endpoint, requests) = runtime(answers);
    let service = Service::connect_with(settings(&endpoint, Duration::from_millis(40)))
        .expect("a connection");
    let started = Instant::now();
    let heartbeat = service.start_heartbeat().expect("the heartbeat");
    std::thread::sleep(Duration::from_millis(150));
    service
        .read(&["P1.level"])
        .expect("a read beside the heartbeat");
    std::thread::sleep(Duration::from_millis(150));
    drop(heartbeat);
    let elapsed = started.elapsed();
    let beats = of_kind(&requests, "service.heartbeat").len();
    let most = usize::try_from(elapsed.as_millis() / 40).unwrap_or(usize::MAX) + 1;
    assert!(
        (2..=most).contains(&beats),
        "{beats} heartbeats in {elapsed:?}, at most {most}"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        of_kind(&requests, "service.heartbeat").len(),
        beats,
        "no heartbeat after the drop"
    );
}

#[test]
fn a_late_heartbeat_is_not_followed_by_the_ones_it_missed() {
    // The runtime answers the first heartbeat 200 ms late, five periods of 40 ms. The next
    // heartbeat comes a period after that answer, so the runtime receives it at least 40 ms after
    // it sent the late answer, however busy the computer is: the heartbeat thread reads the answer
    // after it is sent and then waits a period. Sending the missed ones would send the next four
    // as soon as the answer came. A third heartbeat follows, so the heartbeat goes on. The test
    // waits for the heartbeats instead of counting them in a fixed time, which a busy computer
    // stretches (macOS CI, #628: one heartbeat in 417 ms). Catches a heartbeat that bursts after a
    // stall, which a task does not do either (specification 10 §4.3), and one that stops after it.
    let first = Arc::new(AtomicBool::new(true));
    let beats: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
    let (endpoint, _requests) = runtime({
        let first = Arc::clone(&first);
        let beats = Arc::clone(&beats);
        move |request| {
            if request["type"] == "service.heartbeat" {
                if first.swap(false, Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(200));
                }
                // When the runtime answers it: at once, but the first after its 200 ms.
                beats.lock().expect("the heartbeats").push(Instant::now());
            }
            answers(request)
        }
    });
    let service = Service::connect_with(settings(&endpoint, Duration::from_millis(40)))
        .expect("a connection");
    let heartbeat = service.start_heartbeat().expect("the heartbeat");
    let deadline = Instant::now() + Duration::from_secs(10);
    while beats.lock().expect("the heartbeats").len() < 3 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(heartbeat);
    let beats = beats.lock().expect("the heartbeats").clone();
    assert!(
        beats.len() >= 3,
        "{} heartbeats in 10 s: the heartbeat stopped after the late one",
        beats.len()
    );
    let gap = beats[1] - beats[0];
    assert!(
        gap >= Duration::from_millis(40),
        "the heartbeat after the late one came {gap:?} after its answer, within the period of 40 ms"
    );
}

#[test]
fn a_heartbeat_of_no_period_is_refused() {
    // A period of zero, which settings built in code can carry, would send heartbeats without
    // pause. Catches a heartbeat thread that floods the runtime.
    let (endpoint, requests) = runtime(answers);
    let service = Service::connect_with(settings(&endpoint, Duration::ZERO)).expect("a connection");
    let refused = service
        .start_heartbeat()
        .expect_err("no heartbeat of no period");
    assert!(
        refused.to_string().contains("heartbeat is zero"),
        "{refused}"
    );
    assert_eq!(of_kind(&requests, "service.heartbeat"), Vec::<Value>::new());
}

#[test]
fn a_subscription_reads_at_its_period() {
    // Three reads 60 ms apart: the first at once, so the third comes 120 ms or more after the
    // first. Catches reads sent back to back, which would load the runtime at the speed of the
    // link.
    let (endpoint, requests) = runtime(answers);
    let service =
        Service::connect_with(settings(&endpoint, Duration::from_secs(1))).expect("a connection");
    let started = Instant::now();
    let reads: Vec<_> = service
        .subscribe(&["P1.level"], Duration::from_millis(60))
        .take(3)
        .collect();
    assert!(
        started.elapsed() >= Duration::from_millis(120),
        "{:?}",
        started.elapsed()
    );
    assert!(reads.iter().all(Result::is_ok), "{reads:?}");
    assert_eq!(of_kind(&requests, "variables.read").len(), 3);
}

#[test]
fn an_endpoint_that_is_none_is_refused_before_connecting() {
    // Catches a typo in trust.toml that the service would only meet as a failed connection.
    let refused = Service::connect_with(settings("127.0.0.1:47800", Duration::from_secs(1)))
        .expect_err("no control endpoint");
    assert_eq!(
        refused,
        ServiceError::Settings(
            "127.0.0.1:47800 is no control endpoint such as tcp://127.0.0.1:47800".into()
        )
    );
}

#[test]
fn a_typed_handle_reads_and_writes_its_variable_alone() {
    // The bindings' handles: `get` reads one variable and gives it as its Rust type, `set` writes one
    // writable variable as its type's JSON form. Catches a typed read of another path and a write
    // of another form.
    const LEVEL: Variable<f64> = Variable::new("P1.level");
    const TARGET: Writable<f32> = Writable::new("P1.target");
    let (endpoint, requests) = runtime(answers);
    let service =
        Service::connect_with(settings(&endpoint, Duration::from_secs(1))).expect("a connection");
    assert_eq!(service.get(LEVEL), Ok(1.5));
    assert_eq!(service.set(TARGET, &0.1), Ok(1));
    let params: Vec<Value> = requests
        .lock()
        .expect("the requests")
        .iter()
        .map(|request| request["params"].clone())
        .collect();
    assert_eq!(
        params,
        vec![
            json!({"paths": ["P1.level"]}),
            json!({"writes": [{"path": "P1.target", "value": 0.1}]}),
        ]
    );
}
