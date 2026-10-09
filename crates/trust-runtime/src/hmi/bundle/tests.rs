//! Specification 39, §16.1 and §16.4: the HMI bundle and its version.

use serde_json::json;
use sha2::{Digest, Sha256};

use super::HmiBundle;

fn bundle(files: &[(&str, &[u8])]) -> HmiBundle {
    HmiBundle {
        files: files
            .iter()
            .map(|(path, bytes)| (path.to_string(), bytes.to_vec()))
            .collect(),
    }
}

fn temp(name: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "trust-hmi-bundle-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("a folder");
    dir
}

#[test]
fn the_version_is_the_documented_hash_of_the_files_in_path_order() {
    let files = bundle(&[("hmi/pages/b.toml", b"B"), ("hmi/a.toml", b"A")]);
    let expected = format!(
        "{:x}",
        Sha256::digest(b"hmi/a.toml\0A\0hmi/pages/b.toml\0B\0")
    );
    assert_eq!(files.version(), expected);
    assert_eq!(files.short_version(), expected[..16]);
    assert_ne!(
        bundle(&[("hmi/a.toml", b"A2")]).version(),
        bundle(&[("hmi/a.toml", b"A")]).version()
    );
}

#[test]
fn a_project_bundle_holds_every_file_under_hmi() {
    let root = temp("read");
    std::fs::create_dir_all(root.join("hmi/pages")).expect("pages");
    std::fs::create_dir_all(root.join("hmi/assets")).expect("assets");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(root.join("hmi/screen.toml"), "format = 1\n").expect("screen");
    std::fs::write(root.join("hmi/pages/overview.toml"), "title").expect("page");
    std::fs::write(
        root.join("hmi/assets/pump.png"),
        [0x89, b'P', b'N', b'G', 0xff],
    )
    .expect("asset");
    std::fs::write(root.join("src/main.st"), "PROGRAM").expect("source");
    let read = HmiBundle::read(&root).expect("readable");
    assert_eq!(
        read.files.keys().collect::<Vec<_>>(),
        [
            "hmi/assets/pump.png",
            "hmi/pages/overview.toml",
            "hmi/screen.toml"
        ]
    );
    assert_eq!(
        read.files["hmi/assets/pump.png"],
        vec![0x89, b'P', b'N', b'G', 0xff]
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn text_travels_as_text_and_other_files_in_base64() {
    let files = bundle(&[
        ("hmi/screen.toml", b"format = 1\n"),
        ("hmi/assets/pump.png", &[0x89, 0x50, 0xff]),
    ]);
    let sent = files.to_json();
    assert_eq!(sent["format"], json!(1));
    assert_eq!(sent["version"], json!(files.version()));
    assert_eq!(sent["files"]["hmi/screen.toml"], json!("format = 1\n"));
    assert_eq!(
        sent["files"]["hmi/assets/pump.png"],
        json!({ "base64": "iVD/" })
    );
    assert_eq!(HmiBundle::from_json(&sent), Ok(files));
}

#[test]
fn a_bundle_that_does_not_match_its_version_or_leaves_hmi_is_refused() {
    let files = bundle(&[("hmi/screen.toml", b"format = 1\n")]);
    let mut changed = files.to_json();
    changed["files"]["hmi/screen.toml"] = json!("format = 2\n");
    let refused = |value: serde_json::Value, words: &str| {
        let error = HmiBundle::from_json(&value).expect_err("refused");
        assert!(error.contains(words), "{words}: {error}");
    };
    refused(changed, "version");
    for path in [
        "hmi/../src/main.st",
        "/etc/passwd",
        "src/main.st",
        "hmi\\\\pages\\\\x.toml",
        "hmi/",
    ] {
        let mut outside = json!({ "format": 1, "files": { path: "x" } });
        let unsafe_bundle = HmiBundle {
            files: [(path.to_string(), b"x".to_vec())].into_iter().collect(),
        };
        outside["version"] = json!(unsafe_bundle.version());
        refused(outside, "path");
    }
    refused(json!({ "format": 2, "version": "", "files": {} }), "format");
}

#[test]
fn a_written_bundle_reads_back_the_same() {
    let root = temp("write");
    let files = bundle(&[
        ("hmi/screen.toml", b"format = 1\n"),
        ("hmi/pages/overview.toml", b"title = \"O\"\n"),
        ("hmi/assets/a.bin", &[0, 1, 2]),
    ]);
    files.write(&root).expect("written");
    assert_eq!(HmiBundle::read(&root).expect("readable"), files);
    std::fs::remove_dir_all(root).ok();
}

/// Specification 39, §8.7 item 3: the bundle holds the shipped faceplates and pairs of the
/// project's libraries, under `hmi/`, unless the project has its own of that name; nothing else
/// of a library goes with it.
#[test]
fn a_project_bundle_holds_its_libraries_panels_pairs_and_symbols_its_own_taking_their_place() {
    let project = temp("libraries");
    let write = |path: &str, text: &str| {
        let file = project.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(file, text).expect("write");
    };
    write(
        "trust-lsp.toml",
        "[dependencies]\nequipment = { path = \"lib/equipment\" }\n",
    );
    write(
        "lib/equipment/trust-lsp.toml",
        "[package]\nversion = \"0.1.0\"\n",
    );
    write("lib/equipment/src/equipment.st", "(* sources *)\n");
    write("lib/equipment/hmi/panels/pump.toml", "shipped pump");
    write("lib/equipment/hmi/panels/valve.toml", "shipped valve");
    write("lib/equipment/hmi/pairs/pump.toml", "shipped pair");
    write("lib/equipment/hmi/notes.md", "not a faceplate");
    write("lib/equipment/hmi/symbols/rotor.svg", "<svg/>");
    write("lib/equipment/hmi/symbols/rotor.toml", "shipped rotor");
    write("lib/equipment/hmi/symbols/stator.toml", "shipped stator");
    write("lib/equipment/hmi/symbols/sketch.png", "not a symbol file");
    write("hmi/screen.toml", "screen");
    write("hmi/panels/valve.toml", "own valve");
    write("hmi/symbols/stator.toml", "own stator");
    let read = HmiBundle::read(&project).expect("the bundle");
    let text = |path: &str| {
        read.files
            .get(path)
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
    };
    assert_eq!(
        text("hmi/panels/pump.toml").as_deref(),
        Some("shipped pump")
    );
    assert_eq!(text("hmi/pairs/pump.toml").as_deref(), Some("shipped pair"));
    assert_eq!(
        text("hmi/panels/valve.toml").as_deref(),
        Some("own valve"),
        "the project's own in its place"
    );
    assert_eq!(text("hmi/symbols/rotor.svg").as_deref(), Some("<svg/>"));
    assert_eq!(
        text("hmi/symbols/rotor.toml").as_deref(),
        Some("shipped rotor"),
        "a library's symbol, its drawing and its definition"
    );
    assert_eq!(
        text("hmi/symbols/stator.toml").as_deref(),
        Some("own stator"),
        "the project's own symbol in place of the library's"
    );
    assert_eq!(
        text("hmi/notes.md"),
        None,
        "only panels, pairs and symbols come from a library"
    );
    assert_eq!(text("hmi/symbols/sketch.png"), None);
    assert_eq!(
        read.files.len(),
        7,
        "{:?}",
        read.files.keys().collect::<Vec<_>>()
    );
}
