//! A Rust POU that runs a sequence declares its state (specification 48 §15, RU-71, RU-72): the
//! variable `_seq` last, of the structure `<POU>_SEQ`, and the refusals of a POU and a block that do
//! not belong together. Every expected text is worked out by hand from RU-71.

use super::tests::{errors, project, MANIFEST};
use super::{declarations_text, read};

/// RU-70's palletizer with a local that lives across its wait points.
const PALLETIZER: &str = "use trust_plc::prelude::*;

#[trust_program(name = \"PALLETIZER\", sequence = \"run\")]
#[budget(\"80us\")]
pub struct Palletizer {
    #[input] pub part_present: bool,
    #[output] pub close_gripper: bool,
}

#[trust_sequence]
impl Palletizer {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        let mut parts: i16 = 0;
        loop {
            s.until(|me: &Self| me.part_present).await;
            parts = checked::add(s, parts, 1)?;
            self.close_gripper = true;
            s.wait(Time::from_millis(s, 500)?).await;
            self.close_gripper = false;
        }
    }
}
";

#[test]
fn a_pou_that_runs_a_sequence_declares_its_state_last() {
    // Specification 48 RU-71: the state `_seq` is the POU's last variable, of the structure
    // PALLETIZER_SEQ, which holds the wait point, the time its wait began and the local `parts`,
    // and which the generated code builds from `PalletizerSequence` as any structure of the crate.
    // Catches a declaration without the state, whose program would call the POU with one variable
    // too few, a state that is not last, and a state without its locals.
    let root = project(&[
        ("rust/Cargo.toml", MANIFEST),
        ("rust/src/lib.rs", PALLETIZER),
    ]);
    let krate = read(&root).expect("the crate reads").expect("a crate");
    let names: Vec<&str> = krate.pous[0]
        .pou
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();
    assert_eq!(names, ["part_present", "close_gripper", "_seq"]);
    let state: Vec<(&str, Vec<String>)> = krate
        .structures
        .iter()
        .map(|found| (found.structure.name.as_str(), found.path.clone()))
        .collect();
    assert_eq!(
        state,
        [("PALLETIZER_SEQ", vec!["PalletizerSequence".to_string()])]
    );
    let text = declarations_text(&krate);
    assert!(
        text.contains("VAR\n    _seq : PALLETIZER_SEQ;\nEND_VAR\nEND_PROGRAM\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "TYPE\n    PALLETIZER_SEQ :\n    STRUCT\n        point : UINT;\n        began : TIME;\n        \
             parts : INT;\n    END_STRUCT;\nEND_TYPE\n"
        ),
        "{text}"
    );
}

#[test]
fn a_pou_without_its_sequence_and_a_sequence_no_pou_runs_are_refused() {
    // RU-71: a POU names the sequence its `#[trust_sequence]` block holds, and a block's sequence is
    // a POU's. Catches a POU declared with a state no code fills, and a sequence that never runs.
    let root = project(&[
        ("rust/Cargo.toml", MANIFEST),
        (
            "rust/src/lib.rs",
            "use trust_plc::prelude::*;\n\n#[trust_program(name = \"CELL\", sequence = \"cycle\")]\n#[budget(\"10us\")]\npub struct Cell {\n    #[input] pub start: bool,\n}\n\n#[trust_program(name = \"OTHER\")]\n#[budget(\"10us\")]\npub struct Other {\n    #[input] pub start: bool,\n}\n\n#[trust_sequence]\nimpl Other {\n    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {\n        loop {\n            s.next_cycle().await;\n        }\n    }\n}\n",
        ),
    ]);
    let found = errors(&root);
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found.iter().any(|error| error.starts_with("rust/src/lib.rs:5:12: CELL runs the sequence `cycle`, which no `#[trust_sequence]` block of `Cell` holds")),
        "{found:#?}"
    );
    assert!(
        found.iter().any(|error| error
            .starts_with("rust/src/lib.rs:17:14: the sequence `run` of `Other` runs in no POU")),
        "{found:#?}"
    );
}

/// A press whose sequence waits for both guards with an `s.all` and races its stroke against a
/// stop into the local `won`.
const PRESS: &str = "use trust_plc::prelude::*;

#[trust_program(name = \"PRESS\", sequence = \"run\")]
#[budget(\"80us\")]
pub struct Press {
    #[input] pub left: bool,
    #[input] pub right: bool,
    #[output] pub ended: u16,
}

#[trust_sequence]
impl Press {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.all(s.until(|me: &Self| me.left), s.until(|me: &Self| me.right)).await;
            let won: u16 = s.race(s.until(|me: &Self| me.left), s.next_cycle()).await;
            self.ended = won;
        }
    }
}
";

#[test]
fn a_sequence_that_holds_an_all_declares_done_in_its_state() {
    // RU-71 and RU-72: a body with an s.all has `done : WORD` after `began`, and the local `won`
    // of its s.race after it, so that the generated declaration lays the state out as the macro's
    // `PressSequence` does. Catches a declaration without `done`, whose program would pass the
    // state's locals one field off.
    let root = project(&[("rust/Cargo.toml", MANIFEST), ("rust/src/lib.rs", PRESS)]);
    let krate = read(&root).expect("the crate reads").expect("a crate");
    let text = declarations_text(&krate);
    assert!(
        text.contains(
            "TYPE\n    PRESS_SEQ :\n    STRUCT\n        point : UINT;\n        began : TIME;\n        \
             done : WORD;\n        won : UINT;\n    END_STRUCT;\nEND_TYPE\n"
        ),
        "{text}"
    );
}
