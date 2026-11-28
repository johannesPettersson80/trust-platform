//! Tutorial 15 (a value shared over the mesh, and the peer found with discovery) as its README writes
//! it: two runtimes made of tutorial 13's files and tutorial 15's, PLC-B publishing its lamp and
//! PLC-A subscribing to it.

use super::*;

/// The files of one PLC of tutorial 15: tutorial 13's files, as steps 2 to 5 of its README write
/// them, with the files that tutorial 15 writes into `/tmp/trust-plc-<plc>` in their place.
fn plc_files(plc: char) -> Vec<(String, String)> {
    let readme = |folder: &str| {
        std::fs::read_to_string(repo_root().join(format!("examples/tutorials/{folder}/README.md")))
            .expect("a tutorial's README")
    };
    let mut files = here_documents(&readme("13_project_bootstrap_zero_to_first_app"));
    let prefix = format!("/tmp/trust-plc-{plc}/");
    for (path, text) in here_documents(&readme("15_multi_plc_discovery_mesh")) {
        let Some(file) = path.strip_prefix(&prefix) else {
            continue;
        };
        match files.iter_mut().find(|(existing, _)| existing == file) {
            Some(entry) => entry.1 = text,
            None => files.push((file.to_string(), text)),
        }
    }
    files
}

#[test]
fn tutorial_15_shares_the_lamp_over_the_mesh_and_finds_the_peer_as_its_readme_writes_it() {
    // examples/tutorials/15_multi_plc_discovery_mesh/README.md, worked from its steps (specification
    // 11, the mesh's publish and subscribe, and discovery's command line): PLC-B publishes
    // P1.LampOut, and PLC-A subscribes to it under PLC-B's resource name, PlcB, into P1.RemoteLamp.
    // Step 6 sets PLC-B's %IX0.0 TRUE, then FALSE, and PLC-A's copy follows each time; step 7's
    // search from PLC-A finds PlcB through the mesh and prints it once. The mesh's port is the one
    // place the copies differ from the README: PLC-B's is a free one, and PLC-A connects to it.
    // Catches a subscription that names the publisher otherwise than the mesh does, a value that does
    // not cross, a copy that keeps its first value, and a peer printed at every poll of the search.
    let files = plc_files('b');
    assert!(
        files
            .iter()
            .any(|(file, text)| file == "runtime.toml" && text.contains("name = \"PlcB\"")),
        "PLC-B's runtime.toml from step 3"
    );
    let plc_b = ExampleRuntime::written("tutorial-15-plc-b", files)
        .reserve_mesh_port()
        .keep_discovery()
        .start();
    let connect: &'static str =
        Box::leak(format!("connect = [\"127.0.0.1:{}\"]", plc_b.mesh_port()).into_boxed_str());
    let plc_a = ExampleRuntime::written("tutorial-15-plc-a", plc_files('a'))
        .edit_source("runtime.toml", "connect = [\"127.0.0.1:5212\"]", connect)
        .keep_discovery()
        .start();

    for value in [true, false] {
        set(&plc_b, "%IX0.0", value);
        let expected = value.to_string();
        plc_a.wait_until(
            STEP,
            &format!("step 6: PLC-A's P1.RemoteLamp {value}"),
            || variable(&plc_a, "P1.RemoteLamp") == expected,
        );
        assert_eq!(
            variable(&plc_b, "P1.LampOut"),
            expected,
            "step 6: PLC-B's lamp"
        );
    }

    let endpoint = plc_a.ctl_ok(&["endpoint"]).trim().to_string();
    let searched = trust_runtime_command()
        .args(["discover", "--only", "trust_runtime", "--endpoint"])
        .arg(&endpoint)
        .args(["--token", plc_a.token(), "--deadline-ms", "3000"])
        .output()
        .expect("run trust-runtime discover");
    assert!(searched.status.success(), "step 7: {}", describe(&searched));
    let printed = String::from_utf8_lossy(&searched.stdout);
    let found: Vec<&str> = printed
        .lines()
        .filter(|line| line.starts_with("Found "))
        .collect();
    assert_eq!(found.len(), 1, "step 7 prints PLC-B once:\n{printed}");
    assert!(
        found[0].starts_with("Found PlcB (dev:runtime:")
            && found[0].ends_with("): truST runtimes (confirmed)"),
        "step 7 names PLC-B by its resource name:\n{printed}"
    );
}

/// The data of each event `event` that `runtime` has logged, in the order of the log.
fn logged(runtime: &Runtime, event: &str) -> Vec<serde_json::Value> {
    runtime
        .logged_events()
        .into_iter()
        .filter(|line| line["event"] == event)
        .map(|line| line["data"].clone())
        .collect()
}

/// Whether `runtime` has logged the event `event` for the peer `peer`.
fn logged_for(runtime: &Runtime, event: &str, peer: &str) -> bool {
    logged(runtime, event)
        .iter()
        .any(|data| data["peer"] == peer)
}

/// The token of the Try-it that gives PLC-B's mesh an `auth_token`.
const TRY_IT_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

/// Stops `runtime` as `ctl shutdown --exit` does and waits until its process has ended.
fn stop(runtime: &mut Runtime) {
    runtime.ctl_ok(&["shutdown", "--exit"]);
    let status = runtime.wait_for_exit(STEP);
    assert!(
        status.success(),
        "the runtime ends after shutdown --exit: {status}"
    );
}

/// The README's `connect` line of PLC-A, pointed at `plc_b`'s mesh port.
fn connect_to(plc_b: &Runtime) -> &'static str {
    Box::leak(format!("connect = [\"127.0.0.1:{}\"]", plc_b.mesh_port()).into_boxed_str())
}

#[test]
fn tutorial_15_a_token_on_one_side_leaves_the_peer_missing_until_both_have_it() {
    // examples/tutorials/15_multi_plc_discovery_mesh/README.md, the second Try-it (specification
    // 11, "A mesh links only to the peers it names" and "A mesh says which peers come, go or stay
    // away"). With the token on PLC-B's mesh only, PLC-A cannot join: its lamp stays false with
    // PLC-B's on, and 5 s after its mesh started PLC-A logs one warning mesh_peer_missing for PlcB,
    // with the subscription that waits for it, PlcB:P1.LampOut, and waited_ms 5000, and no join,
    // while PLC-B logs no peer event. With the same token in PLC-A's mesh too, PLC-A logs
    // mesh_peer_joined for PlcB and the lamp crosses. Catches the silent wait of 27 November 2026, a peer told missing more than once, a
    // join through a refused link, and a token that keeps out a runtime that has it.
    let refusing = ExampleRuntime::written("tutorial-15-try-token-b", plc_files('b'))
        .edit_source(
            "runtime.toml",
            "publish = [\"P1.LampOut\"]",
            Box::leak(
                format!("publish = [\"P1.LampOut\"]\nauth_token = \"{TRY_IT_TOKEN}\"")
                    .into_boxed_str(),
            ),
        )
        .reserve_mesh_port()
        .start();
    let mut waiting = ExampleRuntime::written("tutorial-15-try-token-a", plc_files('a'))
        .edit_source(
            "runtime.toml",
            "connect = [\"127.0.0.1:5212\"]",
            connect_to(&refusing),
        )
        .start();
    set(&refusing, "%IX0.0", true);
    waiting.wait_until(STEP, "PLC-A logs PlcB missing", || {
        !logged(&waiting, "mesh_peer_missing").is_empty()
    });
    let missing = logged(&waiting, "mesh_peer_missing");
    assert_eq!(missing.len(), 1, "one missing peer: {missing:?}");
    assert_eq!(missing[0]["peer"], "PlcB", "{missing:?}");
    assert_eq!(
        missing[0]["subscriptions"],
        serde_json::json!(["PlcB:P1.LampOut"]),
        "{missing:?}"
    );
    assert_eq!(missing[0]["waited_ms"], 5000, "{missing:?}");
    assert!(
        logged(&waiting, "mesh_peer_joined").is_empty(),
        "PLC-A cannot join PLC-B's mesh without its token"
    );
    assert!(
        refusing.logged_events().iter().all(|line| !line["event"]
            .as_str()
            .unwrap_or_default()
            .starts_with("mesh_peer")),
        "PLC-B says nothing of the runtime it refuses"
    );
    assert_eq!(variable(&waiting, "P1.RemoteLamp"), "false");

    stop(&mut waiting);
    let settings = waiting.project().join("runtime.toml");
    let mut document: toml::Value =
        toml::from_str(&std::fs::read_to_string(&settings).expect("PLC-A's runtime.toml"))
            .expect("PLC-A's runtime.toml parses");
    document["runtime"]["mesh"]
        .as_table_mut()
        .expect("PLC-A's [runtime.mesh]")
        .insert(
            "auth_token".into(),
            toml::Value::String(TRY_IT_TOKEN.into()),
        );
    std::fs::write(
        &settings,
        toml::to_string(&document).expect("PLC-A's runtime.toml"),
    )
    .expect("write PLC-A's runtime.toml");
    waiting.start_again(&[]);
    waiting.wait_until(STEP, "PLC-A logs PlcB joining with the token", || {
        logged_for(&waiting, "mesh_peer_joined", "PlcB")
    });
    waiting.wait_until(STEP, "the lamp crosses with the token on both", || {
        variable(&waiting, "P1.RemoteLamp") == "true"
    });
}

#[test]
fn tutorial_15_a_misnamed_peer_is_named_missing_and_a_peer_that_goes_and_comes_is_told() {
    // examples/tutorials/15_multi_plc_discovery_mesh/README.md, the first and third Try-its
    // (specification 11, "A mesh says which peers come, go or stay away"). PLC-A subscribing under
    // PLC-B's discovery name, LineB:P1.LampOut, logs mesh_peer_joined for PlcB, keeps its lamp
    // false with PLC-B's on, and 5 s after its start logs one warning mesh_peer_missing for LineB
    // with that subscription. PLC-B stopped, PLC-A logs mesh_peer_left for PlcB; PLC-B started
    // again, PLC-A logs mesh_peer_joined for PlcB a second time. The README names the three events.
    // Catches a misnamed subscription that waits silently, a peer told missing although it joined,
    // a leave or a return left out, and a README that does not say what to look for.
    let readme = std::fs::read_to_string(
        repo_root().join("examples/tutorials/15_multi_plc_discovery_mesh/README.md"),
    )
    .expect("tutorial 15's README");
    for event in ["mesh_peer_joined", "mesh_peer_left", "mesh_peer_missing"] {
        assert!(readme.contains(event), "the README names `{event}`");
    }
    let mut plc_b = ExampleRuntime::written("tutorial-15-try-name-b", plc_files('b'))
        .reserve_mesh_port()
        .start();
    let plc_a = ExampleRuntime::written("tutorial-15-try-name-a", plc_files('a'))
        .edit_source(
            "runtime.toml",
            "connect = [\"127.0.0.1:5212\"]",
            connect_to(&plc_b),
        )
        .edit_source(
            "runtime.toml",
            "\"PlcB:P1.LampOut\" = \"P1.RemoteLamp\"",
            "\"LineB:P1.LampOut\" = \"P1.RemoteLamp\"",
        )
        .start();
    set(&plc_b, "%IX0.0", true);
    plc_a.wait_until(STEP, "PLC-A logs LineB missing", || {
        !logged(&plc_a, "mesh_peer_missing").is_empty()
    });
    assert!(
        logged_for(&plc_a, "mesh_peer_joined", "PlcB"),
        "PLC-A linked to PlcB"
    );
    let missing = logged(&plc_a, "mesh_peer_missing");
    assert_eq!(missing.len(), 1, "one missing peer: {missing:?}");
    assert_eq!(missing[0]["peer"], "LineB", "{missing:?}");
    assert_eq!(
        missing[0]["subscriptions"],
        serde_json::json!(["LineB:P1.LampOut"]),
        "{missing:?}"
    );
    assert_eq!(variable(&plc_a, "P1.RemoteLamp"), "false");

    stop(&mut plc_b);
    plc_a.wait_until(STEP, "PLC-A logs PlcB leaving", || {
        logged_for(&plc_a, "mesh_peer_left", "PlcB")
    });
    plc_b.start_again(&[]);
    plc_a.wait_until(STEP, "PLC-A logs PlcB joining again", || {
        logged(&plc_a, "mesh_peer_joined")
            .iter()
            .filter(|data| data["peer"] == "PlcB")
            .count()
            == 2
    });
}
