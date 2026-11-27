//! Tutorial 11 (testing control logic without hardware) as its README writes it: the tests of a
//! copy of its project run, a bug that a test finds, and the runs of its Try-its. Step 7's run on
//! the runtime is `tutorial_ctl_commands::tutorial_11_level_word_keeps_the_command_bits`.

use super::readme_steps::{self, edit, quoted_answers, shown, test_in, without_times};

/// Tutorial 11's folder under `examples/tutorials`.
const TUTORIAL: &str = "11_unit_testing_102";

/// The test of the band's edges, as the first Try-it writes `src/boundaries.st`.
const BOUNDARIES: &str = "\
TEST_FUNCTION_BLOCK TankControlBoundaries
VAR
    Dut : TankControl;
END_VAR

(* 300 and 700 are the band's own edges: hold. *)
Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#300);
ASSERT_EQUAL(EXPECTED := INT#500, ACTUAL := Dut.PumpSpeedCmd);
Dut(StartCmd := TRUE, StopCmd := FALSE, LevelRaw := INT#700);
ASSERT_EQUAL(EXPECTED := INT#500, ACTUAL := Dut.PumpSpeedCmd);
END_TEST_FUNCTION_BLOCK
";

/// The test that never ends, as the second Try-it writes `src/endless.st`.
const ENDLESS: &str = "\
TEST_PROGRAM EndlessLoop
VAR
    Count : DINT;
END_VAR
WHILE TRUE DO
    Count := Count + 1;
END_WHILE;
END_TEST_PROGRAM
";

/// Whether the README writes `file` with `here_document`, as a step or a Try-it of a list item
/// indented by `indent` spaces does.
fn writes(readme: &str, file: &str, here_document: &str, indent: usize) -> bool {
    let pad = " ".repeat(indent);
    let lines: String = here_document
        .lines()
        .map(|line| {
            if line.is_empty() {
                "\n".to_string()
            } else {
                format!("{pad}{line}\n")
            }
        })
        .collect();
    readme.contains(&format!("cat > {file} <<'ST'\n{lines}{pad}ST\n"))
}

#[test]
fn tutorial_11_runs_and_finds_a_bug_as_its_readme_writes_it() {
    // examples/tutorials/11_unit_testing_102/README.md (specification 22, "Test case execution"
    // and "Test reporting and CI output"). On a copy of the project, step 5's run and step 6's run
    // with the fill speed mistyped as 700 answer what the README quotes, the times aside: the
    // low-level scenario fails at src/tests.st:10, expected 800 and actual 700, and the band's
    // test passes; with 800 back, both pass. Step 7 quotes the outputs that
    // tutorial_ctl_commands::tutorial_11_level_word_keeps_the_command_bits reads on the runtime.
    // Catches a README that quotes what the runner no longer prints, and a failure reported at
    // another line or with other values.
    let readme = readme_steps::readme(TUTORIAL);
    for command in [
        "trust-dev test --project .\n",
        "write `PumpSpeedCmd := INT#700;` in place of\n   `PumpSpeedCmd := INT#800;`",
        "`trust-runtime build --project . --sources src`",
        "trust-runtime ctl --project . io-write %IX0.0 true",
        "trust-runtime ctl --project . io-write %IW2 200",
        "`%QX0.0` (`P1.FillValveOut`) `Bool(true)`, `%QX0.1` (`P1.DrainValveOut`) `Bool(false)`",
        "`%QW2` (`P1.PumpSpeedOut`) `Int(800)`",
        "the pump at `Int(600)`",
        "the pump at `Int(0)`",
    ] {
        assert!(readme.contains(command), "the README has `{command}`");
    }
    let quoted = quoted_answers(&readme);
    assert_eq!(quoted.len(), 2, "steps 5 and 6 quote an answer");
    let project = readme_steps::copied_project(TUTORIAL);
    let run = test_in(&project, &[]);
    edit(
        &project,
        "src/main.st",
        "PumpSpeedCmd := INT#800;",
        "PumpSpeedCmd := INT#700;",
    );
    let broken = test_in(&project, &[]);
    edit(
        &project,
        "src/main.st",
        "PumpSpeedCmd := INT#700;",
        "PumpSpeedCmd := INT#800;",
    );
    let fixed = test_in(&project, &[]);
    let _ = std::fs::remove_dir_all(&project);

    assert_eq!(run.status.code(), Some(0), "step 5: {}", shown(&run));
    assert_eq!(
        without_times(shown(&run).trim_end()),
        without_times(&quoted[0]),
        "step 5's answer"
    );
    assert_eq!(broken.status.code(), Some(1), "step 6: {}", shown(&broken));
    assert_eq!(
        without_times(shown(&broken).trim_end()),
        without_times(&quoted[1]),
        "step 6's answer"
    );
    assert_eq!(fixed.status.code(), Some(0), "800 back: {}", shown(&fixed));
    assert!(shown(&fixed).contains("2 passed, 0 failed, 0 errors"));
}

#[test]
fn tutorial_11_try_its_answer_as_its_readme_writes_them() {
    // examples/tutorials/11_unit_testing_102/README.md, "Try it yourself" (specification 22, "Test
    // case execution"): the test of the band's edges, 300 and 700, passes on the block as shipped
    // and fails at src/boundaries.st:8, expected 500 and actual 800, with `<=` in place of `<` in
    // the fill branch, which the shipped tests miss; a test that never ends stops at the runner's
    // work limit as an error at src/endless.st:6. Catches a README that quotes what the runner no
    // longer prints, edges the block mishandles, and an endless test that hangs the run or ends
    // as anything but an error.
    let readme = readme_steps::readme(TUTORIAL);
    assert!(
        writes(&readme, "src/boundaries.st", BOUNDARIES, 2),
        "the README writes src/boundaries.st as the test does"
    );
    assert!(
        writes(&readme, "src/endless.st", ENDLESS, 2),
        "the README writes src/endless.st as the test does"
    );
    let project = readme_steps::copied_project(TUTORIAL);
    std::fs::write(project.join("src/boundaries.st"), BOUNDARIES).expect("write the edges' test");
    let edges = test_in(&project, &[]);
    edit(
        &project,
        "src/main.st",
        "IF LevelRaw < INT#300 THEN",
        "IF LevelRaw <= INT#300 THEN",
    );
    let slip = test_in(&project, &[]);
    edit(
        &project,
        "src/main.st",
        "IF LevelRaw <= INT#300 THEN",
        "IF LevelRaw < INT#300 THEN",
    );
    std::fs::remove_file(project.join("src/boundaries.st")).expect("remove the edges' test");
    std::fs::write(project.join("src/endless.st"), ENDLESS).expect("write the endless test");
    let endless = test_in(&project, &["--filter", "Endless"]);
    let _ = std::fs::remove_dir_all(&project);

    assert_eq!(edges.status.code(), Some(0), "{}", shown(&edges));
    assert!(shown(&edges).contains("3 passed, 0 failed, 0 errors"));
    let slip_text = shown(&slip);
    assert_eq!(slip.status.code(), Some(1), "{slip_text}");
    for quoted in [
        "FAIL [1/3] TEST_FUNCTION_BLOCK::TankControlBoundaries src/boundaries.st:8",
        "ASSERT_EQUAL failed: expected 500, actual 800",
    ] {
        assert!(
            slip_text.contains(quoted),
            "the run shows `{quoted}`: {slip_text}"
        );
        assert!(
            readme.contains(&format!("`{quoted}`")),
            "the README quotes `{quoted}`"
        );
    }
    let endless_text = shown(&endless);
    assert_eq!(endless.status.code(), Some(1), "{endless_text}");
    for quoted in [
        "ERROR [1/1] TEST_PROGRAM::EndlessLoop src/endless.st:6",
        "reason   : execution timed out: work limit 100000000, used 100000000, requested 1",
    ] {
        assert!(
            endless_text.contains(quoted),
            "the run shows `{quoted}`: {endless_text}"
        );
        assert!(
            readme.contains(&format!("`{quoted}`")),
            "the README quotes `{quoted}`"
        );
    }
}
