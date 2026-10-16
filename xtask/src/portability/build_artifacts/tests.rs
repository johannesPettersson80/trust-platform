use super::*;

const BIN: &str = "trust-nucleo-f401re";

fn record(fresh: bool, executable: Value) -> String {
    serde_json::json!({"reason":"compiler-artifact", "target":{"name":"trust-nucleo-f401re", "kind":["bin"]}, "fresh":fresh, "executable":executable}).to_string()
}

#[test]
fn only_a_new_exact_firmware_binary_can_supply_the_executable() {
    let path = std::env::temp_dir().join("firmware with spaces");
    let current = record(false, serde_json::json!(path));
    assert_eq!(executable(&current, BIN).unwrap(), path);
    assert!(executable(&record(true, serde_json::json!(path)), BIN).is_err());
    assert!(executable(&record(false, Value::Null), BIN).is_err());
    assert!(executable(&record(false, serde_json::json!("relative.elf")), BIN).is_err());
    assert!(executable(&format!("{current}\n{current}"), BIN).is_err());
    assert!(executable("not cargo JSON", BIN).is_err());
    assert!(executable("{\"reason\":\"build-finished\",\"success\":true}", BIN).is_err());
    assert!(executable(
        &current.replace("trust-nucleo-f401re", "another-binary"),
        BIN
    )
    .is_err());
    assert!(executable(&current.replace("\"bin\"", "\"lib\""), BIN).is_err());
}

#[test]
fn map_output_changes_only_the_final_binary_link_destination() {
    let artifacts = Artifacts {
        report: PathBuf::new(),
        elf: PathBuf::new(),
        cargo_log: PathBuf::new(),
        map: std::env::temp_dir().join("new map with spaces"),
    };
    let mut flags = vec![
        "-C".into(),
        "link-arg=--icf=safe".into(),
        "-C".into(),
        "link-arg=-Map=old.map".into(),
        "-C".into(),
        "link-arg=-Tprofile.x".into(),
    ];
    let binary = artifacts.map_flags(&mut flags).unwrap();
    assert_eq!(
        flags,
        ["-C", "link-arg=--icf=safe", "-C", "link-arg=-Tprofile.x"]
    );
    assert_eq!(
        binary,
        [
            "-C".into(),
            format!("link-arg=-Map={}", artifacts.map.display())
        ]
    );
    assert!(artifacts.map_flags(&mut flags).is_err());
    let mut duplicates = vec![
        "-C".into(),
        "link-arg=-Map=a".into(),
        "-C".into(),
        "link-arg=-Map=b".into(),
    ];
    assert!(artifacts.map_flags(&mut duplicates).is_err());
}

#[test]
fn existing_evidence_is_not_overwritten() {
    let parent = std::env::temp_dir().join(format!(
        "trust-link-evidence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let artifacts = Artifacts::prepare(&parent.join("build.json")).unwrap();
    let board = BoardProfile::nucleo_f401re();
    assert!(artifacts
        .retain(
            &record(false, serde_json::json!(parent.join("missing.elf"))),
            &board
        )
        .is_err());
    fs::write(&artifacts.map, b"original map").unwrap();
    assert!(Artifacts::prepare(&artifacts.report).is_err());
    assert_eq!(fs::read(&artifacts.map).unwrap(), b"original map");
    let executable = parent.join("linked.elf");
    fs::write(&executable, b"linked executable").unwrap();
    let retained = artifacts
        .retain(&record(false, serde_json::json!(executable)), &board)
        .unwrap();
    assert_eq!(fs::read(&artifacts.elf).unwrap(), b"linked executable");
    assert_eq!(
        retained["elf_sha256"],
        crate::portability::digest(b"linked executable")
    );
    assert_eq!(
        retained["map_sha256"],
        crate::portability::digest(b"original map")
    );
    fs::remove_dir_all(parent).unwrap();
}

#[cfg(unix)]
#[test]
fn dangling_evidence_symlinks_are_occupied_destinations() {
    let parent = std::env::temp_dir().join(format!(
        "trust-link-symlink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let artifacts = Artifacts::prepare(&parent.join("build.json")).unwrap();
    let missing = parent.join("missing-target");
    for path in [
        &artifacts.report,
        &artifacts.map,
        &artifacts.elf,
        &artifacts.cargo_log,
    ] {
        std::os::unix::fs::symlink(&missing, path).unwrap();
        assert!(Artifacts::prepare(&artifacts.report).is_err());
        assert!(!missing.exists());
        fs::remove_file(path).unwrap();
    }
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn executable_is_selected_by_the_board_binary_name() {
    let path = std::env::temp_dir().join("c6 firmware");
    let c6 = serde_json::json!({
        "reason": "compiler-artifact",
        "target": {"name": "trust-esp32c6-reference", "kind": ["bin"]},
        "fresh": false,
        "executable": path
    })
    .to_string();
    assert_eq!(executable(&c6, "trust-esp32c6-reference").unwrap(), path);
    assert!(executable(&c6, BIN).is_err());
}
