//! `trust-dev rust audit` against the RustSec advisory database (specification 48 RU-79). Every
//! expected value is worked out from the specification: an advisory names a version that matches
//! none of its `patched` and `unaffected` requirements, a withdrawn one names none, a vulnerability
//! is an error and an informational advisory a warning.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use semver::Version;

use super::{check, download, kept_database, unpack, Advisory};

/// A temporary folder of its own for each test.
fn folder(name: &str) -> PathBuf {
    let path = trust_test_support::unique_temp_path(&format!("trust-dev-audit-{name}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// An advisory file as the database writes one: the TOML front matter, then the title.
fn advisory(front: &str, title: &str) -> String {
    format!("```toml\n{front}\n```\n\n# {title}\n\nWhat happens and what to do.\n")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

#[test]
fn the_front_matter_and_the_title_are_read() {
    // Catches a title taken from the front matter or the body, and a lost informational kind.
    let text = advisory(
        "[advisory]\nid = \"RUSTSEC-2020-0071\"\npackage = \"time\"\ndate = \"2020-11-18\"\n\
         informational = \"unsound\"\n\n[versions]\npatched = [\">= 0.2.23\"]\nunaffected = [\"< 0.1.0\"]",
        "Potential segfault in the time crate",
    );
    assert_eq!(
        Advisory::parse(&text).unwrap(),
        Advisory {
            id: "RUSTSEC-2020-0071".into(),
            title: "Potential segfault in the time crate".into(),
            informational: Some("unsound".into()),
            withdrawn: false,
            patched: vec![">= 0.2.23".into()],
            unaffected: vec!["< 0.1.0".into()],
        }
    );
}

#[test]
fn an_advisory_names_the_versions_outside_its_patched_and_unaffected_requirements() {
    // patched >= 0.2.23, unaffected < 0.1.0: 0.1.45 matches neither, so it is named; 0.2.23 and
    // 1.0.0 are patched; 0.0.9 is unaffected. Catches reading only `patched`, and reading
    // `unaffected` as the affected versions.
    let text = advisory(
        "[advisory]\nid = \"RUSTSEC-2020-0071\"\npackage = \"time\"\n\n[versions]\n\
         patched = [\">= 0.2.23\"]\nunaffected = [\"< 0.1.0\"]",
        "Potential segfault in the time crate",
    );
    let advisory = Advisory::parse(&text).unwrap();
    assert_eq!(advisory.names(&version("0.1.45")), Ok(true));
    assert_eq!(advisory.names(&version("0.2.23")), Ok(false));
    assert_eq!(advisory.names(&version("1.0.0")), Ok(false));
    assert_eq!(advisory.names(&version("0.0.9")), Ok(false));
}

#[test]
fn an_advisory_without_requirements_names_every_version_and_a_withdrawn_one_none() {
    // An unmaintained crate has no patched version: every version is named. A withdrawn advisory
    // names none, even a version its requirements would name. Catches an empty `patched` read as
    // "every version patched", and a withdrawn advisory still counted.
    let unmaintained = Advisory::parse(&advisory(
        "[advisory]\nid = \"RUSTSEC-2099-0002\"\npackage = \"libm\"\ninformational = \"unmaintained\"\n\n\
         [versions]\npatched = []",
        "libm is unmaintained",
    ))
    .unwrap();
    assert_eq!(unmaintained.names(&version("0.2.15")), Ok(true));
    let withdrawn = Advisory::parse(&advisory(
        "[advisory]\nid = \"RUSTSEC-2099-0003\"\npackage = \"helper\"\nwithdrawn = \"2099-02-01\"\n\n\
         [versions]\npatched = [\">= 9.0.0\"]",
        "Not a vulnerability after all",
    ))
    .unwrap();
    assert!(withdrawn.withdrawn);
    assert_eq!(withdrawn.names(&version("0.3.1")), Ok(false));
}

#[test]
fn the_audit_reports_a_vulnerability_as_an_error_and_an_informational_advisory_as_a_warning() {
    // helper 0.3.1: RUSTSEC-2099-0001 (patched >= 0.3.2) names it, RUSTSEC-2099-0004 (patched
    // >= 0.3.0) does not; libm 0.2.15: the unmaintained notice names it; ryu has no advisory.
    // Catches a patched advisory reported, a warning reported as an error, and a crate checked
    // against another crate's advisories.
    let db = folder("report");
    write(
        &db.join("crates/helper/RUSTSEC-2099-0001.md"),
        &advisory(
            "[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"helper\"\n\n[versions]\n\
             patched = [\">= 0.3.2\"]",
            "Helper reads past its buffer",
        ),
    );
    write(
        &db.join("crates/helper/RUSTSEC-2099-0004.md"),
        &advisory(
            "[advisory]\nid = \"RUSTSEC-2099-0004\"\npackage = \"helper\"\n\n[versions]\n\
             patched = [\">= 0.3.0\"]",
            "Helper's old parser",
        ),
    );
    write(
        &db.join("crates/libm/RUSTSEC-2099-0002.md"),
        &advisory(
            "[advisory]\nid = \"RUSTSEC-2099-0002\"\npackage = \"libm\"\ninformational = \"unmaintained\"\n\n\
             [versions]\npatched = []",
            "libm is unmaintained",
        ),
    );
    let graph = vec![
        ("helper".to_string(), "0.3.1".to_string()),
        ("libm".to_string(), "0.2.15".to_string()),
        ("ryu".to_string(), "1.0.20".to_string()),
    ];
    let lines: Vec<String> = check(&graph, &db)
        .unwrap()
        .iter()
        .map(super::Finding::line)
        .collect();
    assert_eq!(
        lines,
        [
            "error: RUSTSEC-2099-0001 helper 0.3.1: Helper reads past its buffer (patched: >= 0.3.2)",
            "warning: RUSTSEC-2099-0002 libm 0.2.15, unmaintained: libm is unmaintained (patched: none)",
        ]
    );
}

#[test]
fn a_folder_without_advisories_is_no_database() {
    // Catches a wrong --db folder that checks nothing and reports no vulnerability.
    let empty = folder("empty");
    let error = check(&[("helper".into(), "0.3.1".into())], &empty).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("holds no RustSec advisory database"),
        "{error}"
    );
}

/// A gzipped tar of `files`, each a path and its text.
fn archive(files: &[(&str, &str)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (path, text) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(text.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, text.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn the_archive_gives_only_the_advisories_of_crates() {
    // Of the archive, only `crates/<crate>/<id>.md` below the top folder is an advisory of a
    // crate: the README, the standard library's advisories and a crate's other files are not.
    // Catches files written outside `crates/` and the top folder kept in the paths.
    let text = advisory(
        "[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"helper\"",
        "Helper",
    );
    let bytes = archive(&[
        ("advisory-db-main/README.md", "# RustSec"),
        ("advisory-db-main/crates/helper/RUSTSEC-2099-0001.md", &text),
        ("advisory-db-main/crates/helper/notes.txt", "notes"),
        ("advisory-db-main/rust/std/RUSTSEC-2000-0001.md", &text),
    ]);
    let into = folder("unpack");
    assert_eq!(unpack(&bytes, &into).unwrap(), 1);
    assert_eq!(
        std::fs::read_to_string(into.join("crates/helper/RUSTSEC-2099-0001.md")).unwrap(),
        text
    );
    assert!(!into.join("README.md").exists());
    assert!(!into.join("crates/helper/notes.txt").exists());
    assert!(!into.join("rust").exists());
    assert!(!into.join("advisory-db-main").exists());
}

/// A web server on this computer that answers `/advisory-db.tar.gz` with `archive` and anything
/// else with 404, for `requests` requests; gives its address.
fn server(archive: Vec<u8>, requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().take(requests) {
            let mut stream = stream.unwrap();
            stream.set_nonblocking(false).unwrap();
            // The whole request before the answer, so that no system resets the connection.
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                request.push(byte[0]);
            }
            let found = String::from_utf8_lossy(&request).starts_with("GET /advisory-db.tar.gz ");
            let (status, body): (&str, &[u8]) = if found {
                ("200 OK", &archive)
            } else {
                ("404 Not Found", b"")
            };
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = stream.write_all(body);
        }
    });
    address
}

#[test]
fn a_download_replaces_the_kept_copy_and_a_failed_one_falls_back_to_it() {
    // A download keeps the archive's advisories in advisory-db/ with the time it was downloaded;
    // a failed one leaves that copy as it was, and the audit then checks against it, saying when
    // it was downloaded; with no copy kept, the failure stops the audit. Catches a failed download
    // that removes the kept copy, and a fallback that hides when the copy was downloaded.
    let text = advisory(
        "[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"helper\"\n\n[versions]\n\
         patched = [\">= 0.3.2\"]",
        "Helper reads past its buffer",
    );
    let address = server(
        archive(&[("advisory-db-main/crates/helper/RUSTSEC-2099-0001.md", &text)]),
        4,
    );
    let data = folder("data");
    download(&format!("{address}/advisory-db.tar.gz"), &data).unwrap();
    let kept = data.join("advisory-db");
    assert_eq!(
        std::fs::read_to_string(kept.join("crates/helper/RUSTSEC-2099-0001.md")).unwrap(),
        text
    );
    assert!(kept.join("downloaded").is_file());
    // 1763469900 s after 1970-01-01 is 18 November 2025, 12:45 UTC (`date -u -d @1763469900`).
    std::fs::write(kept.join("downloaded"), "1763469900").unwrap();
    let missing = format!("{address}/missing.tar.gz");
    assert!(download(&missing, &data).is_err());
    assert!(kept.join("crates/helper/RUSTSEC-2099-0001.md").is_file());
    assert_eq!(
        kept_database(&missing, &data).unwrap(),
        (kept, "downloaded 2025-11-18 12:45 UTC".to_string())
    );
    let error = kept_database(&missing, &folder("no-copy")).unwrap_err();
    assert!(
        format!("{error:#}").contains("no copy is kept"),
        "{error:#}"
    );
}
