//! The facts the `Value` store left in the store differential, kept as the expected side of the tests
//! after the store was deleted (specification 34 §6.2; checklist C1-19e).
//!
//! Until C1-19e these tests built each program on the `Value` store and on the flat store and required
//! the same facts from both: every declared variable, the process images, the latched fault, the
//! debugger's copy of the storage, the result of each scan and the work and bytes charged at each of its
//! phases, the result and charge of each engineering operation, the call stack a debugger reads and the
//! retained payload. In the build that deleted the `Value` store, [`record_the_expected_facts`] recorded
//! the flat store's facts while the differential, still in that build, found them equal line by line to
//! the `Value` store's: for each set of facts, its key, its number of lines and the FNV-1a digest of its
//! lines, one file for the stack tier and one for builds with the register tier. The flat store must
//! leave facts of the same digests, and a set that differs is reported with the lines the flat store
//! left. A change that alters the facts on purpose records them again with that test, and the diff of
//! the recorded file names each set that changed.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// The recorded facts of this build.
#[cfg(not(feature = "register-ir"))]
const RECORDED: &str = include_str!("expected/stack.facts");
#[cfg(feature = "register-ir")]
const RECORDED: &str = include_str!("expected/register-ir.facts");

/// The recorded file of this build, relative to the crate.
#[cfg(not(feature = "register-ir"))]
const RECORDED_PATH: &str = "src/vm/engine/store_construction_tests/expected/stack.facts";
#[cfg(feature = "register-ir")]
const RECORDED_PATH: &str = "src/vm/engine/store_construction_tests/expected/register-ir.facts";

/// The lines of a set that differs shown in its report.
const SHOWN_LINES: usize = 200;

/// The number of lines and the FNV-1a (64-bit) digest of a set of facts, over each line followed by a
/// line feed.
fn digest(lines: &[String]) -> (usize, u64) {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for line in lines {
        for byte in line.bytes().chain(core::iter::once(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    (lines.len(), hash)
}

/// Where the facts of a test go: compared with the recorded file, or recorded.
pub(super) struct Facts {
    recording: bool,
    recorded: BTreeMap<String, (usize, u64)>,
    seen: BTreeMap<String, (usize, u64)>,
    differences: Vec<String>,
}

impl Facts {
    /// Facts compared with the recorded file.
    pub(super) fn compared() -> Self {
        let mut recorded = BTreeMap::new();
        for line in RECORDED.lines() {
            let mut fields = line.rsplitn(3, '\t');
            let (Some(hash), Some(count), Some(key)) =
                (fields.next(), fields.next(), fields.next())
            else {
                panic!("a recorded line is a key, a count and a digest: {line}");
            };
            let count = count.parse().expect("a recorded count");
            let hash = u64::from_str_radix(hash, 16).expect("a recorded digest");
            assert!(
                recorded.insert(key.to_owned(), (count, hash)).is_none(),
                "the key {key} is recorded once"
            );
        }
        Self {
            recording: false,
            recorded,
            seen: BTreeMap::new(),
            differences: Vec::new(),
        }
    }

    /// Facts recorded, for [`record_the_expected_facts`].
    fn recorded() -> Self {
        Self {
            recording: true,
            recorded: BTreeMap::new(),
            seen: BTreeMap::new(),
            differences: Vec::new(),
        }
    }

    /// Check the set of facts `lines` under `key`: record it, or compare it with the recorded set.
    /// Answers whether it agrees; a set that differs is kept, with its lines, for [`Facts::finish`].
    pub(super) fn check(&mut self, key: &str, lines: &[String]) -> bool {
        assert!(
            !key.contains(['\t', '\n']),
            "a key is one field of one line: {key:?}"
        );
        let found = digest(lines);
        if self.seen.insert(key.to_owned(), found).is_some() {
            self.differences.push(format!("{key}: checked twice"));
            return false;
        }
        if self.recording {
            return true;
        }
        match self.recorded.get(key) {
            Some(expected) if *expected == found => true,
            expected => {
                // A set can be long (a sweep's refusals): its first lines name the difference.
                let shown: Vec<&str> = lines.iter().take(SHOWN_LINES).map(String::as_str).collect();
                self.differences.push(format!(
                    "{key}: recorded {expected:?}, found {found:?}:\n  {}",
                    shown.join("\n  ")
                ));
                false
            }
        }
    }

    /// Record a failure that is not a set of facts, such as a healthy program's scan that faulted.
    pub(super) fn fail(&mut self, failure: String) {
        self.differences.push(failure);
    }

    /// Require that every set agreed and, when comparing, that every set whose key starts with
    /// `prefix` was checked, so a test that stopped early does not pass on the sets it skipped.
    pub(super) fn finish(self, prefix: &str) {
        let mut differences = self.differences;
        if !self.recording {
            differences.extend(
                self.recorded
                    .keys()
                    .filter(|key| key.starts_with(prefix) && !self.seen.contains_key(*key))
                    .map(|key| format!("{key}: recorded, not checked")),
            );
        }
        assert_eq!(differences, Vec::<String>::new(), "{prefix}");
    }

    /// Write the recorded file of this build: one line for each set, its key, its number of lines and
    /// its digest in hexadecimal, in key order.
    fn write(self) {
        assert_eq!(
            self.differences,
            Vec::<String>::new(),
            "the recorded tests failed"
        );
        let mut text = String::new();
        for (key, (count, hash)) in &self.seen {
            writeln!(text, "{key}\t{count}\t{hash:016x}").expect("a string takes the line");
        }
        std::fs::write(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(RECORDED_PATH),
            text,
        )
        .expect("the recorded file is written");
    }
}

/// Record the facts of every test that is checked against the recorded file, and write the file of
/// this build. Run it only when a change alters the facts on purpose, and review the diff.
#[test]
#[ignore = "records the expected facts; run it only when a change alters them on purpose"]
fn record_the_expected_facts() {
    let mut facts = Facts::recorded();
    super::scans::record(&mut facts);
    super::engineering::record(&mut facts);
    super::debug_frames::record(&mut facts);
    super::retain::record(&mut facts);
    facts.write();
}
