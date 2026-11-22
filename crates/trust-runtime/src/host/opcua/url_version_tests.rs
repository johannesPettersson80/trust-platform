//! The `url` crates that `trust-runtime` builds with (advisory RUSTSEC-2026-0592, specification 24
//! "Dependency Exceptions"): every url 2.x holds the fix, and url 1.x, which has no fixed release, is
//! reached only through opcua 0.12, which never calls `Url::origin()`, the exception `deny.toml`
//! records.

/// The workspace lock that this build resolved. Included, so that a changed lock rebuilds the test.
const WORKSPACE_LOCK: &str = include_str!("../../../../../Cargo.lock");

/// A locked package: its name, its version and its dependency entries as the lock writes them.
struct Package<'a> {
    name: &'a str,
    version: &'a str,
    dependencies: Vec<&'a str>,
}

/// Every `[[package]]` of `lock`.
fn packages(lock: &str) -> Vec<Package<'_>> {
    let mut packages = Vec::new();
    let mut lines = lock.lines().map(str::trim).peekable();
    while let Some(line) = lines.next() {
        if line != "[[package]]" {
            continue;
        }
        let mut package = Package {
            name: "",
            version: "",
            dependencies: Vec::new(),
        };
        while let Some(&field) = lines.peek() {
            if field.is_empty() || field == "[[package]]" {
                break;
            }
            lines.next();
            let quoted = |rest: &'static str| {
                field
                    .strip_prefix(rest)
                    .and_then(|value| value.strip_suffix('"'))
            };
            if let Some(name) = quoted(r#"name = ""#) {
                package.name = name;
            } else if let Some(version) = quoted(r#"version = ""#) {
                package.version = version;
            } else if field == "dependencies = [" {
                for entry in lines.by_ref() {
                    if entry == "]" {
                        break;
                    }
                    package
                        .dependencies
                        .push(entry.trim_end_matches(',').trim_matches('"'));
                }
            }
        }
        packages.push(package);
    }
    packages
}

/// `version` as (major, minor, patch).
fn numbers(version: &str) -> (u64, u64, u64) {
    let mut parts = version.split('.').map(|part| {
        part.parse()
            .unwrap_or_else(|_| panic!("url {version} is not a plain major.minor.patch version"))
    });
    let mut next = || parts.next().unwrap_or(0);
    (next(), next(), next())
}

// Worked by hand from RUSTSEC-2026-0592: url 2.5.8 and earlier take the origin of a nested blob: URL
// by calling `Url::origin()` on it again, without a bound, so one crafted URL overflows the stack;
// 2.5.9 is the first fixed version, and the 1.x line has no fixed release. The lock held 2.5.8 and
// 1.7.2 until 22 November 2026; opcua 0.12.0 depends on 1.7.2 and calls no `origin()`. Catches a
// dependency update or a merged lock that brings an affected url 2.x back, and a new dependent of
// url 1.x, for which the exception would not hold.
#[test]
fn every_locked_url_holds_the_origin_fix_or_is_reached_only_through_opcua() {
    let packages = packages(WORKSPACE_LOCK);
    let urls: Vec<&Package<'_>> = packages
        .iter()
        .filter(|package| package.name == "url")
        .collect();
    assert!(
        !urls.is_empty(),
        "the workspace lock lists no url, which trust-runtime depends on"
    );
    for url in urls {
        let (major, minor, patch) = numbers(url.version);
        if major >= 2 {
            assert!(
                (major, minor, patch) >= (2, 5, 9),
                "url {} is affected by RUSTSEC-2026-0592",
                url.version
            );
            continue;
        }
        let entry = format!("url {}", url.version);
        let dependents: Vec<&str> = packages
            .iter()
            .filter(|package| {
                package
                    .dependencies
                    .iter()
                    .any(|dependency| *dependency == entry)
            })
            .map(|package| package.name)
            .collect();
        assert_eq!(
            dependents,
            ["opcua"],
            "url {} is reached only through opcua, as the exception of RUSTSEC-2026-0592 says",
            url.version
        );
    }
}
