use super::*;

pub(super) fn post_control(
    base: &str,
    request_type: &str,
    params: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut payload = json!({
        "id": 1u64,
        "type": request_type,
    });
    if let Some(params) = params {
        payload["params"] = params;
    }
    let mut response = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(&payload.to_string())
        .expect("post control request");
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read control response body");
    serde_json::from_str(&body).expect("parse control response body")
}

pub(super) fn websocket_url(base: &str) -> String {
    let authority = base.strip_prefix("http://").unwrap_or(base);
    format!("ws://{authority}/ws/hmi")
}

pub(super) fn wait_for_ws_event<S>(
    socket: &mut tungstenite::WebSocket<S>,
    expected_type: &str,
    timeout: Duration,
) -> serde_json::Value
where
    S: Read + Write,
{
    let deadline = Instant::now() + timeout;
    loop {
        let message = match socket.read() {
            Ok(message) => message,
            Err(tungstenite::Error::Io(err))
                if matches!(err.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
            {
                if Instant::now() >= deadline {
                    break;
                }
                continue;
            }
            Err(err) => panic!("read websocket message: {err}"),
        };
        if !message.is_text() {
            if Instant::now() >= deadline {
                break;
            }
            continue;
        }
        let payload: serde_json::Value = serde_json::from_str(
            message
                .into_text()
                .expect("websocket text payload")
                .as_str(),
        )
        .expect("parse websocket payload");
        if payload
            .get("type")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value == expected_type)
        {
            return payload;
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    panic!("timed out waiting for websocket event type {expected_type}");
}

pub(super) fn configure_ws_read_timeout(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
) {
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("set websocket read timeout");
    }
}

pub(super) fn percentile_ms(samples: &[u128], percentile: usize) -> u128 {
    assert!(!samples.is_empty(), "samples must not be empty");
    assert!(percentile <= 100, "percentile must be <= 100");
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = ((sorted.len() - 1) * percentile) / 100;
    sorted[rank]
}

pub(super) fn hmi_fixture_source() -> &'static str {
    r#"
TYPE MODE : (OFF, AUTO); END_TYPE

PROGRAM Main
VAR
    run : BOOL := TRUE;
    // @hmi(min=0, max=100)
    speed : REAL := 42.5;
    mode : MODE := MODE#AUTO;
    name : STRING := 'pump';
END_VAR
END_PROGRAM
"#
}

pub(super) fn build_hmi_script_bundle(js_path: &Path) -> PathBuf {
    let root = js_path.parent().expect("hmi.js parent");
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut bundled = String::new();
    for module_path in HMI_MODULE_PATHS {
        let relative = module_path.trim_start_matches('/');
        let module_file = root.join(relative.strip_prefix("hmi/").unwrap_or(relative));
        let content = repository_source_tree_read_to_string!(
            (&module_file, &repository_root),
            roots = ["crates/trust-runtime/src/web/ui"],
            extension = "js",
        )
        .unwrap_or_else(|_| panic!("read {}", module_file.display()));
        bundled.push_str(content.as_str());
        bundled.push('\n');
    }
    bundled.push_str(
        repository_source_tree_read_to_string!(
            (js_path, &repository_root),
            roots = ["crates/trust-runtime/src/web/ui"],
            extension = "js",
        )
        .expect("read hmi.js")
        .as_str(),
    );

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let bundle_path = std::env::temp_dir().join(format!("trust-hmi-script-bundle-{unique}.js"));
    fs::write(&bundle_path, bundled).expect("write hmi script bundle");
    bundle_path
}

pub(super) fn run_node_hmi_script(js_path: &Path, script: &str, context: &str) {
    let bundle_path = build_hmi_script_bundle(js_path);
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .env("HMI_JS_PATH", &bundle_path)
        .output()
        .expect("run node script");
    fs::remove_file(bundle_path).ok();
    assert!(
        output.status.success(),
        "node script failed ({context}): status={:?}, stdout={}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) const HMI_MODULE_PATHS: [&str; 10] = [
    "/hmi/modules/hmi-model-descriptor.js",
    "/hmi/modules/hmi-model-layout.js",
    "/hmi/modules/hmi-model-navigation.js",
    "/hmi/modules/hmi-model.js",
    "/hmi/modules/hmi-renderers.js",
    "/hmi/modules/hmi-widgets.js",
    "/hmi/modules/hmi-trends-alarms.js",
    "/hmi/modules/hmi-process-view.js",
    "/hmi/modules/hmi-transport.js",
    "/hmi/modules/hmi-pages.js",
];
