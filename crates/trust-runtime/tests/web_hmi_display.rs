//! Specification 39, §11.1: the runtime display — pages at `/hmi`, their data, and the live-value
//! stream — against a running program.

use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use tungstenite::{Message, WebSocket};

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

#[test]
fn a_custom_symbols_numeric_binding_can_supply_the_project_format() {
    let definition = "format = 1\ntitle = \"Reading\"\nsize = [160, 80]\n[properties]\nreading = \"number\"\n[[part]]\nid = \"number\"\nshow = \"reading\"\n";
    let drawing = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 160 80\"><text id=\"number\" x=\"10\" y=\"40\">0</text></svg>";
    let source = "format = 1\ntitle = \"Source\"\n[[object]]\nid = \"custom\"\ntype = \"my:reading\"\nat = [0, 0]\nsize = [160, 80]\nunit = \"rpm\"\ndecimals = 2\n[object.bind]\nreading = \"Main.P101.Speed\"\n";
    let root = project(
        "custom-format",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/source.toml", source),
            ("hmi/pages/overview.toml", OVERVIEW),
            ("hmi/symbols/reading.toml", definition),
            ("hmi/symbols/reading.svg", drawing),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let data = get_json(&running.base, "/api/hmi/page?name=overview");
    assert_eq!(
        data["formats"]["main.p101.speed"],
        json!({ "unit": "rpm", "decimals": 2 })
    );
}

#[test]
fn pages_and_popups_serve_project_value_formats_without_changing_authored_models() {
    let source = OVERVIEW.replace(
        "label = \"Pump speed\"",
        "label = \"Pump speed\"\nunit = \"rpm\"\ndecimals = 1",
    );
    let popup = "format = 1\ntitle = \"Speed\"\nsize = [480, 320]\n[[object]]\nid = \"speed\"\ntype = \"value\"\nat = [0, 0]\nsize = [240, 96]\n[object.bind]\nvalue = \"Main.P101.Speed\"\n";
    let root = project(
        "project-formats",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/source.toml", &source),
            ("hmi/pages/overview.toml", OVERVIEW),
            ("hmi/popups/speed.toml", popup),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    for address in ["/api/hmi/page?name=overview", "/api/hmi/popup?name=speed"] {
        let data = get_json(&running.base, address);
        assert_eq!(
            data["formats"]["main.p101.speed"],
            json!({ "unit": "rpm", "decimals": 1 }),
            "{address}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(root.join("hmi/pages/overview.toml")).unwrap(),
        OVERVIEW
    );
}

fn get(base: &str, path: &str) -> (u16, String, String) {
    let mut response = ureq::get(&format!("{base}{path}"))
        .config()
        .http_status_as_error(false)
        .build()
        .call()
        .unwrap_or_else(|error| panic!("GET {path}: {error}"));
    let status = response.status().as_u16();
    let kind = response
        .headers()
        .get("Content-Type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.body_mut().read_to_string().unwrap_or_default();
    (status, kind, body)
}

fn get_json(base: &str, path: &str) -> Value {
    let (status, _, body) = get(base, path);
    assert_eq!(status, 200, "GET {path}: {body}");
    serde_json::from_str(&body)
        .unwrap_or_else(|error| panic!("GET {path} is JSON: {error}: {body}"))
}

/// Opens the value stream, or returns the HTTP status that refused it.
fn open_stream(base: &str, query: &str) -> Result<WebSocket<TcpStream>, u16> {
    let address = base.trim_start_matches("http://");
    let stream = TcpStream::connect(address).expect("connect to the web server");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("a read timeout");
    match tungstenite::client(format!("ws://{address}/ws/hmi{query}"), stream) {
        Ok((socket, _)) => Ok(socket),
        Err(tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response))) => {
            Err(response.status().as_u16())
        }
        Err(error) => panic!("the stream handshake failed: {error}"),
    }
}

fn next_message(socket: &mut WebSocket<TcpStream>) -> Value {
    loop {
        match socket.read().expect("a stream message") {
            Message::Text(text) => {
                return serde_json::from_str(text.as_str()).expect("a JSON message");
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected stream message: {other:?}"),
        }
    }
}

#[test]
fn the_stream_sends_the_update_period_then_the_page_values_with_the_cycle_count() {
    let root = hmi_project("stream");
    let running = start(&root, WebAuthMode::Local, None);
    let mut socket = open_stream(&running.base, "?page=overview").expect("the stream opens");

    // The period with the stale limit, three periods (§5.3 item 2): the page computes no limit.
    assert_eq!(
        next_message(&mut socket),
        json!({ "period_ms": 250, "stale_ms": 750 })
    );
    let mut first = next_message(&mut socket);
    // The first read may come before the first scan; wait for one taken after it.
    while first["values"]["Main.P101.Running"]["value"] != json!(true) {
        first = next_message(&mut socket);
    }
    assert_eq!(
        first["values"],
        json!({
            "Main.P101.Speed": { "value": 1460.0 },
            "Main.P101.Running": { "value": true },
            "Main.Levels": { "value": [10, 20, 30], "lowerBounds": [1] },
            "Main.Gone": null,
        })
    );
    // The stream also sends the session and the alarm list (specification 40, §5.1).
    let mut second = next_message(&mut socket);
    while second.get("cycle").is_none() {
        second = next_message(&mut socket);
    }
    assert!(
        second["cycle"].as_u64() > first["cycle"].as_u64(),
        "the cycle count advances: {first} then {second}"
    );
}

/// The next simulation message of the stream that `wanted` accepts: whether simulation is on, the
/// configured speed and the achieved speed, as `control.status` reports them (specification 46,
/// §6.1, item 6).
fn next_simulation(socket: &mut WebSocket<TcpStream>, wanted: impl Fn(&Value) -> bool) -> Value {
    loop {
        let message = next_message(socket);
        if let Some(simulation) = message.get("simulation")
            && wanted(simulation)
        {
            return simulation.clone();
        }
    }
}

#[test]
fn the_stream_reports_simulation_off_when_the_runtime_runs_no_simulation() {
    let root = hmi_project("stream-simulation-off");
    let running = start(&root, WebAuthMode::Local, None);
    let mut socket = open_stream(&running.base, "?page=overview").expect("the stream opens");
    assert_eq!(
        next_simulation(&mut socket, |_| true),
        json!({ "enabled": false, "time_scale": 1, "achieved_time_scale": null, "speed_configured": null, "speed_achieved": null })
    );
}

#[test]
fn the_stream_reports_the_simulation_speed_and_each_change_to_it_while_it_runs() {
    let root = hmi_project("stream-simulation-speed");
    let running = start(&root, WebAuthMode::Local, None);
    set_simulation(&running, true, 7);
    set_achieved_speed(&running, Some(6.5));
    let mut socket = open_stream(&running.base, "?page=overview").expect("the stream opens");
    assert_eq!(
        next_simulation(&mut socket, |_| true),
        json!({ "enabled": true, "time_scale": 7, "achieved_time_scale": 6.5, "speed_configured": 7, "speed_achieved": "6.5" })
    );
    // A change to the running simulation reaches the page at its next update (§6.1, item 5). The
    // achieved speed is set first, so no message can show the new speed with the old one.
    set_achieved_speed(&running, Some(9.8));
    set_simulation(&running, true, 10);
    assert_eq!(
        next_simulation(&mut socket, |simulation| simulation["time_scale"]
            == json!(10)),
        json!({ "enabled": true, "time_scale": 10, "achieved_time_scale": 9.8, "speed_configured": 10, "speed_achieved": null })
    );
}

#[test]
fn the_stream_sends_the_flow_its_pipes_show() {
    // A pipe's flow binding is one of its page's values, as an object's binding is (§6, item 6).
    let page = "format = 1\ntitle = \"Flow\"\n\n[[object]]\nid = \"p-101\"\ntype = \"pump\"\nat = [40, 40]\nsize = [128, 108]\n\n[[object]]\nid = \"t-101\"\ntype = \"tank\"\nat = [400, 40]\nsize = [160, 200]\n\n[[pipe]]\nfrom = \"p-101.outlet\"\nto = \"t-101.inlet\"\n\n[pipe.bind]\nflow = \"Main.P101.Running\"\n";
    let root = project(
        "stream-pipe",
        &[("hmi/screen.toml", SCREEN), ("hmi/pages/flow.toml", page)],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let mut socket = open_stream(&running.base, "?page=flow").expect("the stream opens");
    let mut update = next_message(&mut socket);
    for _ in 0..20 {
        if update["values"]["Main.P101.Running"]["value"] == json!(true) {
            break;
        }
        update = next_message(&mut socket);
    }
    assert_eq!(
        update["values"]["Main.P101.Running"],
        json!({ "value": true }),
        "{update}"
    );
}

#[test]
fn a_bound_path_the_running_program_does_not_have_is_listed_as_missing() {
    let root = hmi_project("stream-missing");
    let running = start(&root, WebAuthMode::Local, None);
    let mut socket = open_stream(&running.base, "?page=overview").expect("the stream opens");
    let update = next_update(&mut socket);
    assert_eq!(update["values"]["Main.Gone"], Value::Null, "{update}");
    assert_eq!(
        update["missing"],
        json!(["Main.Gone"]),
        "only the path the program lacks: {update}"
    );
    // The list comes again only when it changes (§18 phase 5 item 4).
    let next = next_update(&mut socket);
    assert!(next.get("missing").is_none(), "unchanged: {next}");
}

/// The next update: a message with the runtime's cycle count.
fn next_update(socket: &mut WebSocket<TcpStream>) -> Value {
    loop {
        let message = next_message(socket);
        if message.get("cycle").is_some() {
            return message;
        }
    }
}

/// A counter that changes every scan beside a value that never does.
const CHANGING: &str = r#"
PROGRAM Main
VAR
    Count : DINT;
    Fixed : DINT := 7;
END_VAR
    Count := Count + 1;
END_PROGRAM
"#;

/// Specification 39, §11.1 item 2 and §18 phase 5 item 4: the stream sends every value once, then
/// only the values that changed, and the cycle count alone when none did.
#[test]
fn after_the_first_update_the_stream_sends_only_the_values_that_changed() {
    let page = "format = 1\ntitle = \"Changes\"\n\n[[object]]\nid = \"count\"\ntype = \"value\"\nat = [0, 0]\nsize = [200, 60]\n\n[object.bind]\nvalue = \"Main.Count\"\n\n[[object]]\nid = \"fixed\"\ntype = \"value\"\nat = [0, 100]\nsize = [200, 60]\n\n[object.bind]\nvalue = \"Main.Fixed\"\n";
    let still = "format = 1\ntitle = \"Still\"\n\n[[object]]\nid = \"fixed\"\ntype = \"value\"\nat = [0, 0]\nsize = [200, 60]\n\n[object.bind]\nvalue = \"Main.Fixed\"\n";
    let root = project(
        "stream-changes",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/changes.toml", page),
            ("hmi/pages/still.toml", still),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        trust_runtime::hmi::access::config::HmiAccessConfig::default(),
        CHANGING,
        None,
    );
    let mut socket = open_stream(&running.base, "?page=changes").expect("the stream opens");
    let first = next_update(&mut socket);
    assert_eq!(
        first["values"]["Main.Fixed"],
        json!({ "value": 7 }),
        "{first}"
    );
    assert!(first["values"].get("Main.Count").is_some(), "{first}");
    let second = next_update(&mut socket);
    assert!(
        second["values"].get("Main.Count").is_some(),
        "the counter changed: {second}"
    );
    assert!(
        second["values"].get("Main.Fixed").is_none(),
        "the fixed value did not: {second}"
    );

    let mut socket = open_stream(&running.base, "?page=still").expect("the stream opens");
    let first = next_update(&mut socket);
    assert_eq!(
        first["values"]["Main.Fixed"],
        json!({ "value": 7 }),
        "{first}"
    );
    let second = next_update(&mut socket);
    assert_eq!(
        second["values"],
        json!({}),
        "nothing changed: the cycle count alone: {second}"
    );
    assert!(
        second["cycle"].as_u64() > first["cycle"].as_u64(),
        "{first} then {second}"
    );
}

#[test]
fn the_stream_refuses_a_page_that_does_not_exist_does_not_load_or_is_not_a_page_name() {
    let root = project(
        "refused",
        &[
            ("hmi/screen.toml", SCREEN),
            (
                "hmi/pages/broken.toml",
                "format = 1\ntitle = \"Broken\"\ncolour = \"red\"\n",
            ),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    assert_eq!(open_stream(&running.base, "?page=missing").err(), Some(404));
    assert_eq!(open_stream(&running.base, "?page=broken").err(), Some(422));
    assert_eq!(
        open_stream(&running.base, "?page=..%2Fsecret").err(),
        Some(400)
    );
    assert_eq!(open_stream(&running.base, "").err(), Some(400));
}

#[test]
fn pages_are_served_at_their_addresses_with_their_data() {
    let root = hmi_project("pages");
    let running = start(&root, WebAuthMode::Local, None);

    for path in ["/hmi", "/hmi/", "/hmi/overview", "/hmi/trends"] {
        let (status, kind, body) = get(&running.base, path);
        assert_eq!(status, 200, "GET {path}");
        assert!(kind.starts_with("text/html"), "GET {path}: {kind}");
        assert!(
            body.contains("/hmi/modules/hmi-drawing.js"),
            "GET {path}: {body}"
        );
    }

    let data = get_json(&running.base, "/api/hmi/page?name=overview");
    assert_eq!(data["pages"], json!(["overview", "trends"]));
    assert_eq!(data["screen"]["size"], json!([1280, 720]));
    assert_eq!(data["screen"]["theme"], json!("dark"));
    assert_eq!(data["page"]["name"], json!("overview"));
    assert_eq!(data["page"]["model"]["title"], json!("Overview"));
    assert_eq!(
        data["page"]["model"]["object"][0]["bind"]["value"]["path"],
        json!("Main.P101.Speed")
    );
    assert_eq!(data["errors"], json!([]));

    let first = get_json(&running.base, "/api/hmi/page");
    assert_eq!(
        first["page"]["name"],
        json!("overview"),
        "without a name, the first page by name"
    );
}

#[test]
fn a_page_file_that_does_not_load_shows_its_errors_with_file_and_line() {
    let root = project(
        "errors",
        &[
            ("hmi/screen.toml", SCREEN),
            (
                "hmi/pages/broken.toml",
                "format = 1\ntitle = \"Broken\"\ncolour = \"red\"\n",
            ),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let data = get_json(&running.base, "/api/hmi/page?name=broken");
    assert_eq!(data["page"], Value::Null);
    let errors = data["errors"].as_array().expect("a list of errors");
    assert_eq!(errors.len(), 1, "{data}");
    assert_eq!(errors[0]["file"], json!("hmi/pages/broken.toml"));
    assert_eq!(errors[0]["line"], json!(3));
}

#[test]
fn a_project_without_a_screen_file_has_no_hmi() {
    let root = project("none", &[("hmi/pages/overview.toml", OVERVIEW)]);
    let running = start(&root, WebAuthMode::Local, None);
    let (status, kind, body) = get(&running.base, "/hmi");
    assert_eq!(status, 200);
    assert!(kind.starts_with("text/html"), "{kind}");
    assert!(body.contains("no HMI"), "{body}");
    assert!(!body.contains("hmi-drawing.js"), "{body}");
    let data = get_json(&running.base, "/api/hmi/page");
    assert_eq!(data["screen"], Value::Null);
    assert_eq!(
        open_stream(&running.base, "?page=overview").err(),
        Some(404)
    );
}

#[test]
fn the_drawing_module_and_the_font_are_served() {
    let root = hmi_project("assets");
    let running = start(&root, WebAuthMode::Local, None);
    let (status, kind, body) = get(&running.base, "/hmi/modules/hmi-drawing.js");
    assert_eq!(status, 200);
    assert!(kind.starts_with("application/javascript"), "{kind}");
    assert!(body.contains("var TrustHmiDrawing"), "the drawing bundle");

    let mut font = ureq::get(&format!(
        "{}/hmi/assets/fonts/InterVariable.woff2",
        running.base
    ))
    .call()
    .expect("the font");
    assert_eq!(
        font.headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok()),
        Some("font/woff2")
    );
    let bytes = font.body_mut().read_to_vec().expect("the font bytes");
    assert_eq!(&bytes[..4], b"wOF2");
}

/// Specification 39, §14.5 item 2: the Chinese, Japanese and Korean fonts are served beside Inter.
#[test]
fn the_chinese_japanese_and_korean_fonts_are_served() {
    let root = hmi_project("cjk-fonts");
    let running = start(&root, WebAuthMode::Local, None);
    for file in [
        "NotoSansSC-Subset.woff2",
        "NotoSansJP-Subset.woff2",
        "NotoSansKR-Subset.woff2",
    ] {
        let mut font = ureq::get(&format!("{}/hmi/assets/fonts/{file}", running.base))
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .unwrap_or_else(|error| panic!("GET {file}: {error}"));
        assert_eq!(font.status().as_u16(), 200, "{file}");
        assert_eq!(
            font.headers()
                .get("Content-Type")
                .and_then(|v| v.to_str().ok()),
            Some("font/woff2"),
            "{file}"
        );
        let bytes = font.body_mut().read_to_vec().expect("the font bytes");
        assert_eq!(&bytes[..4], b"wOF2", "{file}");
        assert!(
            bytes.len() > 500_000,
            "{file} holds a national character set"
        );
    }
}

/// Specification 39, §14.5: the page comes with the text table of the language asked for, when the
/// project has it, else of its first language, and with the project's languages by their names.
#[test]
fn the_page_comes_with_the_texts_of_the_chosen_language() {
    let root = hmi_project("languages");
    let screen = std::fs::read_to_string(root.join("hmi/screen.toml")).expect("the screen");
    std::fs::write(
        root.join("hmi/screen.toml"),
        screen.replace(
            "symbols = \"flat\"\n",
            "symbols = \"flat\"\nlanguages = [\"en\", \"sv\"]\n",
        ),
    )
    .expect("the languages");
    std::fs::create_dir_all(root.join("hmi/texts")).expect("the texts folder");
    std::fs::write(
        root.join("hmi/texts/sv.toml"),
        "format = 1\nlanguage = \"sv\"\n\n[texts]\n\"Overview\" = \"Översikt\"\n",
    )
    .expect("the Swedish texts");
    let running = start(&root, WebAuthMode::Local, None);
    let swedish = get_json(&running.base, "/api/hmi/page?name=overview&lang=sv");
    assert_eq!(swedish["errors"], json!([]), "{swedish}");
    assert_eq!(swedish["language"], "sv");
    assert_eq!(
        swedish["languages"],
        json!([{ "tag": "en", "name": "English" }, { "tag": "sv", "name": "Svenska" }])
    );
    assert_eq!(
        swedish["texts"]["Overview"], "Översikt",
        "the project's own text"
    );
    assert!(
        swedish["texts"]["Running"]
            .as_str()
            .is_some_and(|word| word != "Running"),
        "truST's Swedish word: {}",
        swedish["texts"]["Running"]
    );
    let other = get_json(&running.base, "/api/hmi/page?name=overview&lang=de");
    assert_eq!(
        other["language"], "en",
        "a language the project does not list falls back to its first"
    );
}

/// The local addresses a page refers to in `src="…"` and `href="…"`.
fn referenced(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    for attribute in ["src=\"", "href=\""] {
        let mut rest = html;
        while let Some(start) = rest.find(attribute) {
            rest = &rest[start + attribute.len()..];
            let end = rest.find('"').expect("a closing quote");
            if rest[..end].starts_with('/') {
                found.push(rest[..end].to_string());
            }
            rest = &rest[end..];
        }
    }
    found
}

#[test]
fn the_page_shells_name_their_icon_and_every_file_they_refer_to_is_served() {
    let with_hmi = hmi_project("shell-files");
    let without_hmi = project("shell-files-none", &[]);
    for root in [with_hmi, without_hmi] {
        let running = start(&root, WebAuthMode::Local, None);
        let (_, _, html) = get(&running.base, "/hmi");
        assert!(
            html.contains("rel=\"icon\""),
            "the page names its icon: {html}"
        );
        for path in referenced(&html) {
            let (status, _, _) = get(&running.base, &path);
            assert_eq!(status, 200, "GET {path}");
        }
    }
}

#[test]
fn the_display_module_is_served_for_the_page_shell() {
    let root = hmi_project("display-module");
    let running = start(&root, WebAuthMode::Local, None);
    let (status, kind, body) = get(&running.base, "/hmi/modules/hmi-display.js");
    assert_eq!(status, 200);
    assert!(kind.starts_with("application/javascript"), "{kind}");
    assert!(
        body.contains("/ws/hmi?"),
        "the display bundle opens the value stream"
    );
}

#[test]
fn the_page_data_and_the_stream_need_the_viewer_role() {
    let root = hmi_project("access");
    let running = start(&root, WebAuthMode::Token, Some("secret"));
    let (status, _, _) = get(&running.base, "/api/hmi/page?name=overview");
    assert_eq!(status, 401);
    assert_eq!(
        open_stream(&running.base, "?page=overview").err(),
        Some(401)
    );
    let (status, _, _) = get(&running.base, "/hmi/overview");
    assert_eq!(status, 200, "the page itself holds no program data");
    let authorised = ureq::get(&format!("{}/api/hmi/page?name=overview", running.base))
        .header("X-Trust-Token", "secret")
        .call()
        .expect("an authorised request");
    assert_eq!(authorised.status(), 200);
}

/// Specification 39, §14.4 item 4: a page comes with the project's symbols it places, cleaned,
/// and with no other.
#[test]
fn a_page_comes_with_the_projects_symbols_it_places() {
    let definition = "format = 1\ntitle = \"Agitator\"\nsize = [128, 108]\n\n[properties]\nrunning = \"boolean\"\n\n[[part]]\nid = \"rotor\"\nspin = \"running\"\n";
    let drawing = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><g id=\"rotor\"><rect width=\"8\" height=\"30\"/></g></svg>";
    let root = project(
        "symbols",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/mixing.toml", "format = 1\ntitle = \"Mixing\"\n\n[[object]]\nid = \"mixer\"\ntype = \"my:agitator\"\nat = [0, 0]\nsize = [128, 108]\n\n[object.bind]\nrunning = \"Main.Running\"\n"),
            ("hmi/symbols/agitator.toml", definition),
            ("hmi/symbols/agitator.svg", drawing),
            ("hmi/symbols/spare.toml", definition),
            ("hmi/symbols/spare.svg", drawing),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let data = get_json(&running.base, "/api/hmi/page?name=mixing");
    let symbols = data["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("symbols: {data}"));
    assert_eq!(symbols.len(), 1, "only the symbols the page places: {data}");
    assert_eq!(symbols[0]["name"], json!("agitator"));
    assert_eq!(
        symbols[0]["parts"][0],
        json!({ "part": "rotor", "spin": "running" })
    );
    assert_eq!(
        symbols[0]["svg"]["children"][0]["attrs"]["id"],
        json!("my-agitator-rotor")
    );
    assert_eq!(
        symbols[0]["svg"]["children"][0]["attrs"]["data-part"],
        json!("rotor")
    );
}
