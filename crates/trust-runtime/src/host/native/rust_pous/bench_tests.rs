//! The bench of a project's Rust POUs (specification 48 RU-48): its generated test programs, the
//! hash of what the image is built from, and the record in `trust.toml` `[timing.measured]`. Every
//! expected text is worked out by hand from RU-48 and specification 04.

use super::bench::{programs, BenchProgram, ProgramCase};
use super::measured::{code, write, Measured};
use super::read;
use super::tests::{project, MANIFEST};

/// A function with an in-out and a case, a block in a namespace with two cases and a public
/// method with one, a private method, and a program with a case.
const LIB: &str = "use trust_plc::prelude::*;

#[trust_function(name = \"SCALE\")]
#[budget(\"1us\")]
#[bench_case(gain = 7, total = 3)]
pub fn scale(_cx: &mut Cx, gain: i32, total: InOut<'_, i32>) -> Result<i32, Fault> {
    Ok(gain)
}

#[trust_fb(name = \"Motion.AXIS\")]
#[budget(\"2us\")]
#[bench_case(target = 10.0)]
#[bench_case]
pub struct Axis {
    #[input] pub target: f64,
    #[output] pub done: bool,
}

impl Axis {
    #[trust_method]
    #[budget(\"1us\")]
    #[bench_case(distance = 0.5)]
    pub fn jog(&mut self, _cx: &mut Cx, distance: f64) -> Result<f64, Fault> {
        Ok(distance)
    }

    #[trust_method(private)]
    #[budget(\"1us\")]
    pub fn stop(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[trust_program(name = \"CELL\")]
#[budget(\"5us\")]
#[bench_case(start = true)]
pub struct Cell {
    #[input] pub start: bool,
}
";

#[test]
fn a_bench_calls_each_case_of_each_function_block_and_callable_method_from_a_test_program() {
    // RU-48: a test program for each case, in the order of the crate's POUs and their cases: each
    // named input and each in-out a variable at its value, which the call passes (an input the
    // case does not name left out of the call, so it keeps its initial value), a function's and a
    // method's result taken, a block in its namespace by its own name there, and the block's
    // case without values as a plain call. The private method and the program have none. Catches
    // a case lost or reordered, a value that does not reach the call, an in-out left unbound, and
    // a namespace the type or an INTERNAL method would not be found in.
    let root = project(&[("rust/Cargo.toml", MANIFEST), ("rust/src/lib.rs", LIB)]);
    let krate = read(&root).expect("the crate reads").expect("a crate");
    let bench = programs(&krate);
    assert_eq!(
        bench.source,
        "TEST_PROGRAM TRUST_BENCH_1\n\
         VAR\n    TrustBench1 : DINT := 7;\n    TrustBench2 : DINT := 3;\n    TrustBenchResult : DINT;\nEND_VAR\n\
         TrustBenchResult := SCALE(gain := TrustBench1, total := TrustBench2);\n\
         END_TEST_PROGRAM\n\n\
         NAMESPACE Motion\nTEST_PROGRAM TRUST_BENCH_2\n\
         VAR\n    TrustBench1 : LREAL := 10.0;\n    TrustBenchBlock : AXIS;\nEND_VAR\n\
         TrustBenchBlock(target := TrustBench1);\n\
         END_TEST_PROGRAM\nEND_NAMESPACE\n\n\
         NAMESPACE Motion\nTEST_PROGRAM TRUST_BENCH_3\n\
         VAR\n    TrustBenchBlock : AXIS;\nEND_VAR\n\
         TrustBenchBlock();\n\
         END_TEST_PROGRAM\nEND_NAMESPACE\n\n\
         NAMESPACE Motion\nTEST_PROGRAM TRUST_BENCH_4\n\
         VAR\n    TrustBench1 : LREAL := 0.5;\n    TrustBenchBlock : AXIS;\n    TrustBenchResult : LREAL;\nEND_VAR\n\
         TrustBenchResult := TrustBenchBlock.jog(distance := TrustBench1);\n\
         END_TEST_PROGRAM\nEND_NAMESPACE\n\n"
    );
    let program = |program: &str, pou: &str, case: &str| BenchProgram {
        program: program.into(),
        pou: pou.to_string(),
        case: case.to_string(),
    };
    assert_eq!(
        bench.programs,
        vec![
            program("TRUST_BENCH_1", "SCALE", "gain = 7, total = 3"),
            program("Motion.TRUST_BENCH_2", "Motion.AXIS", "target = 10.0"),
            program("Motion.TRUST_BENCH_3", "Motion.AXIS", ""),
            program("Motion.TRUST_BENCH_4", "Motion.AXIS.jog", "distance = 0.5"),
        ]
    );
    // A program's case gives the inputs the bench writes into its configured instances, each as
    // a Structured Text literal of its type (RU-48).
    assert_eq!(
        bench.program_cases,
        vec![ProgramCase {
            pou: "CELL".to_string(),
            inputs: vec![("start".to_string(), "TRUE".to_string())],
            case: "start = TRUE".to_string(),
        }]
    );
}

/// The project of the code tests: Structured Text, a crate, and a file the build does not read.
fn coded_project() -> std::path::PathBuf {
    project(&[
        ("src/main.st", "PROGRAM Main\nEND_PROGRAM\n"),
        ("rust/Cargo.toml", MANIFEST),
        ("rust/src/lib.rs", LIB),
        ("README.md", "notes\n"),
    ])
}

#[test]
fn the_code_is_the_hash_of_what_the_image_is_built_from() {
    // RU-48: `code` follows the Structured Text, the crate's files and the target, and nothing
    // else: a note beside the project or a build folder in the crate changes nothing. Catches a
    // measurement that stays current after the program changed, and one made stale by a file the
    // image is not built from.
    let root = coded_project();
    let host = "x86_64-unknown-linux-gnu";
    let first = code(&root, host).expect("the code");
    assert_eq!(first.len(), 64, "SHA-256 in hexadecimal: {first}");
    assert_eq!(
        code(&root, host).expect("again"),
        first,
        "the same files give the same code"
    );
    assert_ne!(
        code(&root, "riscv32imac-unknown-none-elf").expect("another target"),
        first
    );
    std::fs::write(root.join("README.md"), "other notes\n").expect("the note");
    std::fs::create_dir_all(root.join("rust/target/debug")).expect("a build folder");
    std::fs::write(root.join("rust/target/debug/out"), "built").expect("a build output");
    assert_eq!(code(&root, host).expect("after the note"), first);
    std::fs::write(
        root.join("src/main.st"),
        "PROGRAM Main\nVAR x : INT; END_VAR\nEND_PROGRAM\n",
    )
    .expect("the program");
    let after_program = code(&root, host).expect("after the program changed");
    assert_ne!(after_program, first, "a change to the Structured Text");
    std::fs::write(root.join("rust/src/lib.rs"), format!("{LIB}\n// changed\n"))
        .expect("the crate");
    assert_ne!(
        code(&root, host).expect("after the crate changed"),
        after_program,
        "a change to the crate"
    );
}

#[test]
fn the_record_replaces_a_targets_entries_and_keeps_the_rest_of_trust_toml() {
    // RU-48: the command writes each measured POU's entry of its target into [timing.measured],
    // keyed as [timing.budgets] is, and keeps every other entry and every comment. Catches a write
    // that drops another target's measurement, the profile or a comment, and one that adds an
    // entry instead of replacing it.
    let root = project(&[(
        "trust.toml",
        "# The line's timing.\n[timing]\nprofile = \"development\"\n\n[timing.measured]\n\
         \"SCALE\" = { \"riscv32imac-unknown-none-elf\" = { longest = 900, code = \"c6\", device = \"a3f0\" }, \
         \"x86_64-unknown-linux-gnu\" = { longest = 1, code = \"old\", device = \"computer\" } }\n",
    )]);
    let host = "x86_64-unknown-linux-gnu";
    let measurement = |pou: &str, longest_ns| Measured {
        pou: pou.to_string(),
        longest_ns,
        code: "new".to_string(),
        device: "computer".to_string(),
    };
    write(
        &root,
        host,
        &[
            measurement("SCALE", 420),
            measurement("Motion.AXIS.jog", 75),
        ],
    )
    .expect("written");
    let text = std::fs::read_to_string(root.join("trust.toml")).expect("trust.toml");
    assert!(
        text.starts_with("# The line's timing.\n[timing]\nprofile = \"development\"\n"),
        "{text}"
    );
    let table: toml::Table = toml::from_str(&text).expect("trust.toml parses");
    let recorded = &table["timing"]["measured"];
    let entry = |pou: &str, target: &str| {
        let record = &recorded[pou][target];
        (
            record["longest"].as_integer().expect("longest"),
            record["code"].as_str().expect("code").to_string(),
            record["device"].as_str().expect("device").to_string(),
        )
    };
    assert_eq!(
        entry("SCALE", host),
        (420, "new".to_string(), "computer".to_string())
    );
    assert_eq!(
        entry("SCALE", "riscv32imac-unknown-none-elf"),
        (900, "c6".to_string(), "a3f0".to_string())
    );
    assert_eq!(
        entry("Motion.AXIS.jog", host),
        (75, "new".to_string(), "computer".to_string())
    );
    let without = project(&[]);
    std::fs::create_dir_all(&without).expect("the project folder");
    write(&without, host, &[measurement("SCALE", 420)]).expect("written without a trust.toml");
    let table: toml::Table =
        toml::from_str(&std::fs::read_to_string(without.join("trust.toml")).expect("trust.toml"))
            .expect("parses");
    assert_eq!(
        table["timing"]["measured"]["SCALE"][host]["longest"].as_integer(),
        Some(420)
    );
}
