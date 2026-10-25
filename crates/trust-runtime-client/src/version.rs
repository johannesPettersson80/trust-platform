//! The version rule of every runtime (specification 34 TARGET-05): the tools read the runtime's
//! truST version over the control connection (`status.version`), and when it is not their own they
//! say which update is needed. A truST release's tools and runtimes belong together, so the older
//! of the two is updated to the newer. Versions are compared as release versions,
//! `major.minor.patch` with an optional pre-release part (`-rc.1`), which comes before the release
//! it names (Semantic Versioning 2.0.0 §11); build metadata (`+...`) is ignored.

use std::cmp::Ordering;
use std::fmt;

/// Which update a runtime and the tools that talk to it need when their versions differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionAdvice {
    /// The runtime is older: update it to the tools' version.
    UpdateRuntime {
        /// The runtime's version.
        runtime: String,
        /// The tools' version, which the runtime is to be updated to.
        tools: String,
    },
    /// The tools are older: update them to the runtime's version.
    UpdateTools {
        /// The runtime's version, which the tools are to be updated to.
        runtime: String,
        /// The tools' version.
        tools: String,
    },
    /// A version that does not read as a release version: update both to the same release.
    Unreadable {
        /// The runtime's version, as it reported it.
        runtime: String,
        /// The tools' version.
        tools: String,
    },
}

impl VersionAdvice {
    /// The advice for a runtime that reports `runtime` to tools of version `tools`; none when both
    /// are the same release.
    #[must_use]
    pub fn for_versions(tools: &str, runtime: &str) -> Option<Self> {
        let (tools, runtime) = (tools.trim(), runtime.trim());
        let advice = |kind: fn(String, String) -> Self| Some(kind(runtime.into(), tools.into()));
        match (Release::parse(tools), Release::parse(runtime)) {
            (Some(own), Some(theirs)) => match theirs.cmp(&own) {
                Ordering::Equal => None,
                Ordering::Less => advice(|runtime, tools| Self::UpdateRuntime { runtime, tools }),
                Ordering::Greater => advice(|runtime, tools| Self::UpdateTools { runtime, tools }),
            },
            _ if tools == runtime => None,
            _ => advice(|runtime, tools| Self::Unreadable { runtime, tools }),
        }
    }

    /// Whether updating the runtime to the tools' version is the update needed: the runtime is
    /// older, or its version cannot be compared.
    #[must_use]
    pub fn updates_runtime(&self) -> bool {
        !matches!(self, Self::UpdateTools { .. })
    }
}

impl fmt::Display for VersionAdvice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpdateRuntime { runtime, tools } => write!(
                f,
                "the runtime runs truST {runtime} and these tools are truST {tools}: update the \
                 runtime to truST {tools}"
            ),
            Self::UpdateTools { runtime, tools } => write!(
                f,
                "the runtime runs truST {runtime} and these tools are truST {tools}: update these \
                 tools to truST {runtime}"
            ),
            Self::Unreadable { runtime, tools } => write!(
                f,
                "the runtime reports truST version '{runtime}', which these tools (truST {tools}) \
                 cannot compare: update both to the same release"
            ),
        }
    }
}

/// A release version: its three numbers and its pre-release identifiers.
#[derive(Debug, PartialEq, Eq)]
struct Release<'a> {
    numbers: [u64; 3],
    pre: Vec<&'a str>,
}

impl<'a> Release<'a> {
    fn parse(text: &'a str) -> Option<Self> {
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut parts = core.split('.');
        let mut numbers = [0; 3];
        for number in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            *number = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(pre) => {
                let identifiers: Vec<&str> = pre.split('.').collect();
                if identifiers.iter().any(|identifier| identifier.is_empty()) {
                    return None;
                }
                identifiers
            }
            None => Vec::new(),
        };
        Some(Self { numbers, pre })
    }
}

impl Ord for Release<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.numbers.cmp(&other.numbers).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // A pre-release comes before the release it names.
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => compare_pre(&self.pre, &other.pre),
            }
        })
    }
}

impl PartialOrd for Release<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Pre-release identifiers in order: numbers numerically and before words, words in ASCII order,
/// and a shorter list before a longer one that begins with it.
fn compare_pre(left: &[&str], right: &[&str]) -> Ordering {
    for (a, b) in left.iter().zip(right) {
        let order = match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same release needs no update; build metadata does not make another release. Catches a
    /// plain text comparison that advises an update for `0.31.0+abc`.
    #[test]
    fn the_same_release_needs_no_update() {
        assert_eq!(VersionAdvice::for_versions("0.31.0", "0.31.0"), None);
        assert_eq!(VersionAdvice::for_versions("0.31.0", " 0.31.0+abc "), None);
    }

    /// The older side is updated to the newer: a runtime at 0.30.9 under tools at 0.31.0 is
    /// updated, tools at 0.31.0 under a runtime at 0.31.1 are, and 0.10.0 is newer than 0.9.0.
    /// Catches a text comparison, which orders `0.10.0` before `0.9.0`, and an update offered the
    /// wrong way, which would install an older runtime over a newer one.
    #[test]
    fn the_older_side_is_updated_to_the_newer() {
        assert_eq!(
            VersionAdvice::for_versions("0.31.0", "0.30.9"),
            Some(VersionAdvice::UpdateRuntime {
                runtime: "0.30.9".into(),
                tools: "0.31.0".into(),
            })
        );
        assert_eq!(
            VersionAdvice::for_versions("0.31.0", "0.31.1"),
            Some(VersionAdvice::UpdateTools {
                runtime: "0.31.1".into(),
                tools: "0.31.0".into(),
            })
        );
        assert!(VersionAdvice::for_versions("0.10.0", "0.9.0")
            .is_some_and(|advice| advice.updates_runtime()));
    }

    /// A pre-release comes before its release, and its identifiers compare numbers numerically:
    /// `0.31.0-rc.2` before `0.31.0`, `rc.2` before `rc.10`, `rc.1` before `rc.1.1`. Catches a
    /// pre-release taken for the newer release.
    #[test]
    fn a_pre_release_comes_before_its_release() {
        assert!(VersionAdvice::for_versions("0.31.0", "0.31.0-rc.2")
            .is_some_and(|advice| advice.updates_runtime()));
        assert!(VersionAdvice::for_versions("0.31.0-rc.10", "0.31.0-rc.2")
            .is_some_and(|advice| advice.updates_runtime()));
        assert!(VersionAdvice::for_versions("0.31.0-rc.1", "0.31.0-rc.1.1")
            .is_some_and(|advice| !advice.updates_runtime()));
    }

    /// A version that is no release version is advised as both to one release, with the texts of
    /// the contract. Catches an unreadable version taken for the same release.
    #[test]
    fn an_unreadable_version_asks_for_one_release() {
        let advice = VersionAdvice::for_versions("0.31.0", "dev").expect("an advice");
        assert!(advice.updates_runtime());
        assert_eq!(
            advice.to_string(),
            "the runtime reports truST version 'dev', which these tools (truST 0.31.0) cannot \
             compare: update both to the same release"
        );
        assert_eq!(
            VersionAdvice::for_versions("0.31.0", "0.30.9")
                .expect("an advice")
                .to_string(),
            "the runtime runs truST 0.30.9 and these tools are truST 0.31.0: update the runtime \
             to truST 0.31.0"
        );
    }
}
