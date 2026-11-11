//! The rustix that `trust-runtime` builds with holds the fix of advisory RUSTSEC-2026-0486.

/// The workspace lock that this build resolved. Included, so that a changed lock rebuilds the test.
const WORKSPACE_LOCK: &str = include_str!("../../../Cargo.lock");

/// The version of every `rustix` that a lock holds, as (major, minor, patch).
fn locked_rustix_versions(lock: &str) -> Vec<(u64, u64, u64)> {
    let mut lines = lock.lines().map(str::trim);
    let mut versions = Vec::new();
    while let Some(line) = lines.next() {
        if line != r#"name = "rustix""# {
            continue;
        }
        let version = lines
            .next()
            .and_then(|line| line.strip_prefix(r#"version = ""#))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("a locked package's version follows its name");
        let number = |part: Option<&str>| {
            part.and_then(|part| part.parse().ok()).unwrap_or_else(|| {
                panic!("rustix {version} is not a plain major.minor.patch version")
            })
        };
        let mut parts = version.split('.');
        versions.push((
            number(parts.next()),
            number(parts.next()),
            number(parts.next()),
        ));
    }
    versions
}

// RUSTSEC-2026-0486 has no exception under specification 24, "Dependency Exceptions", so it must stay fixed: before 1.1.6,
// rustix's extended-attribute functions marked as initialized the bytes that a zero-length buffer never received. The
// advisory names the 1.x series before 1.1.6; the 0.38 series never had that API. Worked by hand from the advisory: the
// 1.1.5 that the lock held until 11 November 2026 is affected, 1.1.6 is the first fixed version, and the lock's 0.38.44
// is not named. Catches a dependency update or a merged lock file that brings an affected rustix back.
#[test]
fn every_locked_rustix_holds_the_fix_of_its_extended_attribute_advisory() {
    let versions = locked_rustix_versions(WORKSPACE_LOCK);
    assert!(
        !versions.is_empty(),
        "the workspace lock lists no rustix, which trust-runtime depends on"
    );
    let affected: Vec<_> = versions
        .iter()
        .filter(|&&(major, minor, patch)| major == 1 && (minor, patch) < (1, 6))
        .collect();
    assert!(
        affected.is_empty(),
        "rustix {affected:?} is affected by RUSTSEC-2026-0486"
    );
}
