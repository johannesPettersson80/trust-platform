use super::*;
use crate::harness::TestHarness;

/// A configuration with two tasks and a program outside every task, a constant structure and a
/// located variable, compiled for this computer.
const FACTS_SOURCE: &str = r#"
TYPE Limits : STRUCT
    Gain : LREAL := 1.0;
END_STRUCT END_TYPE
PROGRAM Loop
VAR CONSTANT Fixed : Limits; END_VAR
VAR
    Tuned : Limits;
    Output AT %QW0 : INT;
END_VAR
END_PROGRAM
PROGRAM Logger
VAR Count : DINT; END_VAR
END_PROGRAM
PROGRAM Idle
VAR Ticks : DINT; END_VAR
END_PROGRAM
CONFIGURATION Plant
RESOURCE Cell ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow(INTERVAL := T#250ms, PRIORITY := 2);
    PROGRAM L1 WITH Fast : Loop;
    PROGRAM Log WITH Slow : Logger;
    PROGRAM Background : Idle;
END_RESOURCE
END_CONFIGURATION
"#;

/// Specification 35 A6.8 (protocol annex E7.1): a member is writable only when no declaration along
/// its path is constant, in-out or located. A member of a constant structure is not, though its own
/// field declaration is not constant; a located variable is not, and names its address. Catches a
/// check that reads only the last declaration of the path, which the symbol of a structure's field
/// carries without its owner's qualifier.
#[test]
fn a_member_of_a_constant_or_located_declaration_takes_no_live_write() {
    let harness = TestHarness::from_source(FACTS_SOURCE).expect("compile the facts fixture");
    harness.runtime().with_program_view(|program| {
        let image = program.program_image();
        assert!(parameter_writable(&image, "L1.Tuned.Gain"));
        let fixed = binding_leaf(&image, "L1.Fixed.Gain").expect("a declared member");
        assert!(fixed.constant, "the owner's CONSTANT reaches its field");
        assert!(!parameter_writable(&image, "L1.Fixed.Gain"));
        let output = binding_leaf(&image, "L1.Output").expect("a declared member");
        assert!(output.direct);
        assert_eq!(
            output.address,
            Some(IoAddress::parse("%QW0").expect("an address"))
        );
        assert!(!parameter_writable(&image, "L1.Output"));
        assert!(
            binding_leaf(&image, "L1.Missing").is_none(),
            "a member the program does not declare names nothing"
        );
        assert!(
            binding_leaf(&image, "L1").is_none(),
            "a program instance alone is no member"
        );
    });
}

/// Specification 35 A2.3 and protocol annex E1.3: the tasks come from the image's task table with
/// their intervals and the program instances they run, the programs outside every task left out.
/// Catches a task list that keeps the background record or loses a task's programs.
#[test]
fn the_tasks_name_their_intervals_and_their_program_instances() {
    let harness = TestHarness::from_source(FACTS_SOURCE).expect("compile the facts fixture");
    harness.runtime().with_program_view(|program| {
        let image = program.program_image();
        assert_eq!(
            program_tasks(&image),
            vec![
                ProgramTask {
                    name: "Fast".into(),
                    interval: Duration::from_millis(10),
                    programs: vec!["L1".into()],
                },
                ProgramTask {
                    name: "Slow".into(),
                    interval: Duration::from_millis(250),
                    programs: vec!["Log".into()],
                },
            ]
        );
        assert!(is_program_instance(&image, "background"));
        assert!(
            !is_program_instance(&image, "Idle"),
            "a program type is no instance"
        );
        assert!(is_global_or_program(&image, "l1"));
        assert!(source_revision(&image).starts_with("sha256:"));
    });
}
