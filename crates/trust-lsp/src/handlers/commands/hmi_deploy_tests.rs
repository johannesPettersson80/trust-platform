/// Deploy and Roll back from the HMI Builder (specification 39, §16.2 item 8, §16.4), carried out
/// by the language server against a runtime on loopback ports: its control endpoint answers
/// `fleet.topology` and its web server answers the deployment.
mod hmi_deploy_tests {
    use super::*;
    use crate::handlers::commands::run_targets::fake_runtime::{closed_endpoint, FakeRuntime};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const SOURCE: &str = "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n";
    const SCREEN: &str = "format = 1\nsize = [1280, 720]\ntheme = \"dark\"\nsymbols = \"flat\"\n";

    /// A page titled `title` whose value display is bound to `path`.
    fn page(title: &str, path: &str) -> String {
        format!(
            "format = 1\ntitle = \"{title}\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"{path}\"\n"
        )
    }

    /// A project with the program above, the screen and `pages` (name, text), and the language
    /// server's view of it.
    fn project(pages: &[(&str, String)]) -> (PathBuf, Url, MockContext) {
        let root = temp_dir("trustlsp-hmi-deploy");
        std::fs::create_dir_all(root.join("src")).expect("create src dir");
        std::fs::create_dir_all(root.join("hmi/pages")).expect("create pages dir");
        std::fs::write(root.join("src/main.st"), SOURCE).expect("write source");
        std::fs::write(root.join("hmi/screen.toml"), SCREEN).expect("write screen");
        for (name, text) in pages {
            std::fs::write(root.join(format!("hmi/pages/{name}.toml")), text).expect("write page");
        }
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        let context = MockContext {
            workspace_configs: vec![(
                root_uri.clone(),
                test_project_config(root.to_string_lossy().as_ref(), "x86_64"),
            )],
            workspace_folders: vec![root_uri.clone()],
            ..MockContext::default()
        };
        (root, root_uri, context)
    }

    /// One request the web server got.
    #[derive(Debug, PartialEq)]
    struct Posted {
        method: String,
        path: String,
        token: Option<String>,
        body: Value,
    }

    /// A runtime's web server on a loopback port that answers every request with one status and
    /// body, and records the requests.
    struct WebServer {
        port: u16,
        posted: Arc<Mutex<Vec<Posted>>>,
    }

    impl WebServer {
        fn answering(status: u16, body: &'static str) -> Self {
            Self::start(status, String::new(), body)
        }

        /// A web server that answers every request with `status`, the headers `headers` (each
        /// line ending in CRLF) and `body`.
        fn start(status: u16, headers: String, body: &'static str) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
            let port = listener.local_addr().expect("the listener's address").port();
            let posted = Arc::new(Mutex::new(Vec::new()));
            let recorded = Arc::clone(&posted);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else {
                        continue;
                    };
                    let mut reader = BufReader::new(stream.try_clone().expect("the reader"));
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("the request line");
                    let mut words = line.split_whitespace();
                    let (method, path) = (words.next().unwrap_or_default().to_owned(), words.next().unwrap_or_default().to_owned());
                    let (mut length, mut token) = (0, None);
                    loop {
                        let mut header = String::new();
                        reader.read_line(&mut header).expect("a header");
                        let Some((name, value)) = header.trim_end().split_once(':') else {
                            break;
                        };
                        match name.to_ascii_lowercase().as_str() {
                            "content-length" => length = value.trim().parse().expect("a length"),
                            "x-trust-token" => token = Some(value.trim().to_owned()),
                            _ => {}
                        }
                    }
                    let mut text = vec![0; length];
                    reader.read_exact(&mut text).expect("the body");
                    let received = serde_json::from_slice(&text).unwrap_or(Value::Null);
                    recorded.lock().expect("the requests").push(Posted { method, path, token, body: received });
                    let answer = format!(
                        "HTTP/1.1 {status} Answer\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(answer.as_bytes()).expect("the answer");
                }
            });
            Self { port, posted }
        }

        fn posted(&self) -> Vec<Posted> {
            std::mem::take(&mut *self.posted.lock().expect("the requests"))
        }
    }

    /// The `fleet.topology` answer of a runtime whose own web server listens on `listen`, or that
    /// has none.
    fn topology(listen: Option<String>) -> Value {
        let endpoints = listen.map_or_else(Vec::new, |listen| {
            vec![json!({ "id": "web", "kind": "service", "protocol": "web", "source": "self",
                         "params": { "listen": listen, "tls": false, "auth": "token" } })]
        });
        json!({ "schema_version": 1, "hosts": [{ "host_id": "h", "runtimes": [{ "runtime_id": "r", "endpoints": endpoints }], "containers": [] }] })
    }

    /// A runtime on a loopback port whose web server listens on every interface at `web`'s port.
    fn runtime_with(web: &WebServer) -> FakeRuntime {
        FakeRuntime::answering(topology(Some(format!("0.0.0.0:{}", web.port))))
    }

    /// The outcome of `trust-lsp.hmiDeploy` for `arguments`, which it reads.
    fn outcome_of(context: &MockContext, arguments: Value) -> Value {
        let answer = hmi_deploy::deploy_value(context, vec![arguments]);
        assert_eq!(answer["ok"], true, "{answer}");
        answer["outcome"].clone()
    }

    fn deploy(context: &MockContext, root_uri: &Url, target: Value) -> Value {
        outcome_of(
            context,
            json!({ "action": "deploy", "project": { "root_uri": root_uri }, "target": target, "authToken": "secret" }),
        )
    }

    fn line_1(endpoint: &str) -> Value {
        json!({ "endpoint": endpoint, "label": "Line 1", "simulator": false })
    }

    /// The bundle of the editor check goes to the web server that the runtime reports for itself,
    /// on the host of its control endpoint since it listens on every interface, with the runtime's
    /// control token on both requests; the outcome is the version the runtime made active. Catches
    /// a bundle sent to 0.0.0.0, without the token, to another path, or other than the checked one.
    #[test]
    fn hmi_deploy_sends_the_checked_bundle_to_the_runtimes_own_web_server() {
        let (root, root_uri, context) = project(&[("overview", page("Overview", "Main.Level"))]);
        let web = WebServer::answering(200, r#"{"ok":true,"active":"abc123","previous":"old999"}"#);
        let runtime = runtime_with(&web);
        let outcome = deploy(&context, &root_uri, line_1(&runtime.endpoint));
        assert_eq!(outcome, json!({ "ok": true, "version": "abc123", "target": "Line 1" }));
        let checked = hmi_checks::bundle_value_with_context(&context, vec![json!({ "root_uri": root_uri })]);
        assert_eq!(checked["problems"], json!([]), "{checked}");
        assert_eq!(
            web.posted(),
            [Posted {
                method: "POST".into(),
                path: "/api/hmi/deploy".into(),
                token: Some("secret".into()),
                body: checked["bundle"].clone(),
            }]
        );
        let requests = runtime.requests();
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert_eq!((&requests[0]["type"], &requests[0]["auth"]), (&json!("fleet.topology"), &json!("secret")));
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    /// The editor check's problems stop the deployment before the runtime is asked anything, and
    /// come back with file and line. Worked by hand: two pages bound to a variable the program does
    /// not have are two problems, each on its page's `value` line, 11. Catches a deployment that
    /// goes ahead with problems and a count of one for two.
    #[test]
    fn hmi_deploy_stops_at_the_editor_checks_problems_and_sends_nothing() {
        let (root, root_uri, context) =
            project(&[("overview", page("Overview", "Main.Gone")), ("trend", page("Trend", "Main.Lost"))]);
        let web = WebServer::answering(200, r#"{"ok":true,"active":"abc123","previous":null}"#);
        let runtime = runtime_with(&web);
        let outcome = deploy(&context, &root_uri, line_1(&runtime.endpoint));
        assert_eq!((&outcome["ok"], &outcome["reason"]), (&json!(false), &json!("The editor check found 2 problems.")));
        let problems = outcome["problems"].as_array().expect("problems");
        let places: Vec<(&Value, &Value)> = problems.iter().map(|problem| (&problem["file"], &problem["line"])).collect();
        assert_eq!(
            places,
            [(&json!("hmi/pages/overview.toml"), &json!(11)), (&json!("hmi/pages/trend.toml"), &json!(11))]
        );
        assert!(problems[0]["message"].as_str().is_some_and(|message| message.contains("Main.Gone")), "{outcome}");
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!((runtime.connections(), web.posted()), (0, Vec::new()));
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    /// Without a runtime, with the simulator, and with a runtime that reports no web server or does
    /// not answer, nothing is sent and the outcome says what to do. Catches a deployment tried
    /// against the simulator, which has no web server, and a failure without its reason.
    #[test]
    fn hmi_deploy_says_what_to_do_without_a_runtime_that_has_a_web_server() {
        let (root, root_uri, context) = project(&[("overview", page("Overview", "Main.Level"))]);
        assert_eq!(
            deploy(&context, &root_uri, Value::Null),
            json!({ "ok": false, "reason": "No runtime is running: start a runtime to deploy to." })
        );
        let simulator = FakeRuntime::answering(topology(None));
        assert_eq!(
            deploy(&context, &root_uri, json!({ "endpoint": simulator.endpoint, "label": "Simulator", "simulator": true })),
            json!({ "ok": false, "reason": "The simulator has no web server: use Live mode to see the page, or start a runtime to deploy to." })
        );
        assert_eq!(simulator.connections(), 0, "the simulator is not asked");
        let without_web = FakeRuntime::answering(topology(None));
        assert_eq!(
            deploy(&context, &root_uri, line_1(&without_web.endpoint)),
            json!({ "ok": false, "reason": "Line 1 has no web server: enable [runtime.web] in its runtime.toml." })
        );
        let silent = deploy(&context, &root_uri, line_1(&closed_endpoint()));
        assert_eq!(silent["ok"], false, "{silent}");
        assert!(silent["reason"].as_str().is_some_and(|reason| reason.starts_with("Line 1 does not answer: ")), "{silent}");
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    /// The runtime's refusal comes back as the runtime gives it: its problems with file and line
    /// (§16.4 item 3), else its error, else its status; a web server that cannot be reached is
    /// named. Catches a refusal shown as a deployment and a refusal's problems dropped.
    #[test]
    fn hmi_deploy_reports_the_runtimes_refusal_with_its_problems() {
        let (root, root_uri, context) = project(&[("overview", page("Overview", "Main.Level"))]);
        let checked = WebServer::answering(
            422,
            r#"{"ok":false,"problems":[{"file":"hmi/pages/overview.toml","line":3,"message":"needs more than the write policy allows"}],"active":"abc123","previous":null}"#,
        );
        assert_eq!(
            deploy(&context, &root_uri, line_1(&runtime_with(&checked).endpoint)),
            json!({ "ok": false, "reason": "Line 1 found 1 problem in the bundle.", "problems": [
                { "file": "hmi/pages/overview.toml", "line": 3, "message": "needs more than the write policy allows" }
            ] })
        );
        let refused = WebServer::answering(403, r#"{"ok":false,"error":"deploying needs the deploy permission"}"#);
        assert_eq!(
            deploy(&context, &root_uri, line_1(&runtime_with(&refused).endpoint)),
            json!({ "ok": false, "reason": "deploying needs the deploy permission" })
        );
        let broken = WebServer::answering(500, "not json");
        assert_eq!(
            deploy(&context, &root_uri, line_1(&runtime_with(&broken).endpoint)),
            json!({ "ok": false, "reason": "Line 1 answered 500" })
        );
        let free = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = free.local_addr().expect("the listener's address").port();
        drop(free);
        let unreachable = FakeRuntime::answering(topology(Some(format!("127.0.0.1:{port}"))));
        let outcome = deploy(&context, &root_uri, line_1(&unreachable.endpoint));
        assert!(outcome["reason"].as_str().is_some_and(|reason| reason.starts_with("Line 1 cannot be reached: ")), "{outcome}");
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    /// A redirect is answered as the runtime gave it and never followed. Catches the runtime's
    /// control token sent to another address.
    #[test]
    fn hmi_deploy_never_follows_a_redirect() {
        let (root, root_uri, context) = project(&[("overview", page("Overview", "Main.Level"))]);
        let elsewhere = WebServer::answering(200, r#"{"ok":true,"active":"abc123","previous":null}"#);
        let location = format!("Location: http://127.0.0.1:{}/api/hmi/deploy\r\n", elsewhere.port);
        let redirecting = WebServer::start(302, location, "{}");
        assert_eq!(
            deploy(&context, &root_uri, line_1(&runtime_with(&redirecting).endpoint)),
            json!({ "ok": false, "reason": "Line 1 answered 302" })
        );
        assert_eq!(redirecting.posted().len(), 1);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(elsewhere.posted(), Vec::new(), "nothing reached the other address");
        std::fs::remove_dir_all(root).expect("remove temp dir");
    }

    /// Roll back asks the web server to make the previous bundle active and builds no bundle; the
    /// outcome is the version now active. Catches a rollback sent to the deploy path or answered
    /// with the version it replaced.
    #[test]
    fn hmi_rollback_makes_the_previous_bundle_active() {
        let web = WebServer::answering(200, r#"{"ok":true,"active":"old999","previous":"abc123"}"#);
        let runtime = runtime_with(&web);
        let rolled_back = outcome_of(
            &MockContext::default(),
            json!({ "action": "rollback", "target": line_1(&runtime.endpoint), "authToken": "secret" }),
        );
        assert_eq!(rolled_back, json!({ "ok": true, "version": "old999", "target": "Line 1" }));
        assert_eq!(
            web.posted(),
            [Posted {
                method: "POST".into(),
                path: "/api/hmi/rollback".into(),
                token: Some("secret".into()),
                body: json!({}),
            }]
        );
    }
}
