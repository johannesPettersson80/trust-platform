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
