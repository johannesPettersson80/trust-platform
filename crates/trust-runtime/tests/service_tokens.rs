//! Service tokens on a computer (specification 11 §6.9.6; specification 48 RU-81): the real
//! `trust-runtime` runs a private copy of the example `memory_marker_counter`, and
//! `trust-runtime ctl service-token` creates the token of the service `recipe` with the copy's Admin
//! token. The service then authenticates with its token for the role the token holds; creating the
//! token again replaces it, and the earlier one no longer authenticates. The copy runs with the debug
//! surface on, so that `pause` and `resume`, which need it, show what an Operator may do.
//!
//! The test needs the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::path::Path;
use std::process::Output;

use serde_json::json;

use runtime_process::{describe, ExampleRuntime, Runtime, CLIMB};

/// `ctl --token <token> <args>` against the copy: the request carries the service's token in place
/// of the copy's Admin token.
fn as_service(runtime: &Runtime, token: &str, args: &[&str]) -> Output {
    let mut all = vec!["--token", token];
    all.extend_from_slice(args);
    runtime.ctl(&all)
}

/// Creates the token of `name` with `role` into `out` and returns the token the file holds, after
/// checking the command's line, which names the token's id, its role and the file, never the token.
fn create(runtime: &Runtime, name: &str, role: Option<&str>, out: &Path) -> String {
    let out_text = out.to_string_lossy().into_owned();
    let mut args = vec!["service-token", name, "--out", &out_text];
    if let Some(role) = role {
        args.extend(["--role", role]);
    }
    let printed = runtime.ctl_ok(&args);
    let token = std::fs::read_to_string(out).expect("the token file");
    assert_eq!(
        printed.trim(),
        format!(
            "id=service-{name} role={} token_file={out_text}",
            role.unwrap_or("viewer")
        )
    );
    assert!(!printed.contains(&token), "the token is never printed");
    // 32 random bytes are 43 characters of URL-safe base64 without padding.
    assert_eq!(token.len(), 43, "{token}");
    token
}

#[test]
fn a_service_authenticates_with_its_token_until_the_token_is_replaced() {
    // The token of `recipe`, Viewer as no role is asked, reads the status and is refused `pause`,
    // an Operator's request; its file is the owner's alone. Created again as `RECIPE` with the role
    // Operator, the same service's token is replaced: the earlier token is refused as an invalid
    // token, the new one pauses and resumes, and `pair.list` holds one row, with the service's
    // name and no expiry. Catches a service token that does not authenticate, one with a role it
    // was not given, and a rotation that leaves the earlier token valid or adds a second row.
    let runtime = ExampleRuntime::new("memory_marker_counter")
        .debug_enabled(true)
        .start();
    let out = runtime.project().join("secrets").join("recipe.token");

    let first = create(&runtime, "recipe", None, &out);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&out)
            .expect("the token file")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
    let status = as_service(&runtime, &first, &["status"]);
    assert!(status.status.success(), "{}", describe(&status));
    let refused = as_service(&runtime, &first, &["pause"]);
    assert!(!refused.status.success(), "a Viewer may not pause");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("insufficient_role"),
        "{}",
        describe(&refused)
    );

    let second = create(&runtime, "RECIPE", Some("operator"), &out);
    assert_ne!(second, first);
    let stale = as_service(&runtime, &first, &["status"]);
    assert!(!stale.status.success(), "the replaced token");
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("invalid_auth_token"),
        "{}",
        describe(&stale)
    );
    for action in ["pause", "resume"] {
        let operated = as_service(&runtime, &second, &[action]);
        assert!(operated.status.success(), "{}", describe(&operated));
    }

    let listed = runtime.control_request(json!({"id": 7, "type": "pair.list"}));
    let tokens = listed["result"]["tokens"].as_array().expect("the tokens");
    assert_eq!(tokens.len(), 1, "{listed}");
    assert_eq!(tokens[0]["id"], json!("service-RECIPE"), "{listed}");
    assert_eq!(tokens[0]["role"], json!("operator"), "{listed}");
    assert_eq!(tokens[0]["service"], json!("RECIPE"), "{listed}");
    assert_eq!(tokens[0]["expires_at"], json!(null), "{listed}");
    assert_eq!(tokens[0]["enabled"], json!(true), "{listed}");
}

/// The value of `recipe_heartbeat` that `variables.read` gives, `None` before the first completed
/// scan.
fn heartbeat_value(runtime: &Runtime) -> Option<u64> {
    let reply = runtime.control_request(json!({
        "id": 9,
        "type": "variables.read",
        "params": { "paths": ["recipe_heartbeat"] }
    }));
    reply["result"]["values"][0]["value"].as_u64()
}

#[test]
fn a_service_heartbeat_reaches_the_programs_global_at_the_next_cycle() {
    // Specification 11 §6.8.1, "Service heartbeats", and specification 48 RU-63: the copy's
    // program declares `recipe_heartbeat : UDINT`, as the build declares a service's heartbeat.
    // The service token of recipe, a Viewer's, sends `service.heartbeat`: the reply names recipe
    // and 1, the global's 0 plus one, and a cycle later `variables.read` gives 1; a second
    // heartbeat gives 2. The copy's Admin token speaks for no service and is refused. Catches a
    // heartbeat the scan never applies, one that adds more than one, and one any client could
    // send.
    let runtime = ExampleRuntime::new("memory_marker_counter")
        .edit_source(
            "src/Configuration.st",
            "CONFIGURATION MarkerCounter\n",
            "VAR_GLOBAL\n    recipe_heartbeat : UDINT;\nEND_VAR\n\nCONFIGURATION MarkerCounter\n",
        )
        .start();
    let out = runtime.project().join("secrets").join("recipe.token");
    let token = create(&runtime, "recipe", None, &out);
    runtime.wait_until(CLIMB, "a completed scan", || {
        heartbeat_value(&runtime) == Some(0)
    });
    for beat in 1..=2_u64 {
        let reply =
            runtime.control_request_as(&token, json!({"id": beat, "type": "service.heartbeat"}));
        assert_eq!(
            reply["result"],
            json!({"service": "recipe", "value": beat}),
            "{reply}"
        );
        runtime.wait_until(CLIMB, "the heartbeat in the global", || {
            heartbeat_value(&runtime) == Some(beat)
        });
    }
    let admin = runtime.control_request(json!({"id": 3, "type": "service.heartbeat"}));
    assert_eq!(
        admin["error"],
        json!("service.heartbeat needs a service token"),
        "{admin}"
    );
}
