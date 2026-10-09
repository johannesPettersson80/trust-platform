//! Specification 39, §8.9 items 1 and 2: a faceplate fits the instances whose block has every
//! field it needs, by its pair or by the field's own name, of a kind the field takes.

use super::*;
use crate::harness::TestHarness;
use crate::hmi::member_paths::find_ignoring_case;
use crate::hmi::page_files::{parse_faceplate, parse_pair};

const PROGRAM: &str = r#"
FUNCTION_BLOCK Pump
VAR_OUTPUT
    Running : BOOL;
    ActualSpeed : REAL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Valve
VAR_OUTPUT
    Open : BOOL;
    Speed : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Station
VAR
    Feed : Pump;
END_VAR
    Feed();
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    V101 : Valve;
    Level : REAL;
    North : Station;
    P102 : Pump;
END_VAR
    P101();
    V101();
    North();
    P102();
END_PROGRAM
"#;

const CARD: &str = "format = 1\ntitle = \"Pump card\"\nsize = [320, 160]\n\n[fields.Running]\naccepts = [\"boolean\"]\n\n[fields.Speed]\naccepts = [\"number\"]\n";

const PAIR: &str = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump-card\"\nsymbol = \"pump\"\n\n[fields]\nSpeed = \"ActualSpeed\"\n";

fn harness() -> TestHarness {
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    harness.cycle();
    harness
}

fn card() -> (HmiFaceplate, Vec<HmiPair>) {
    let faceplate = parse_faceplate("hmi/panels/pump-card.toml", CARD).expect("the card loads");
    let pair = parse_pair("hmi/pairs/pump-card.toml", PAIR).expect("the pair loads");
    (faceplate, vec![pair])
}

#[test]
fn a_faceplate_fits_an_instance_whose_block_has_every_field_by_its_pair_or_its_name() {
    let harness = harness();
    let (faceplate, pairs) = card();
    assert_eq!(
        misfits(&faceplate, &pairs, "Main.P101", harness.runtime().storage()),
        Vec::<String>::new()
    );
}

#[test]
fn a_block_without_a_field_or_with_another_kind_does_not_fit_and_says_which() {
    let harness = harness();
    let (faceplate, pairs) = card();
    let storage = harness.runtime().storage();
    let valve = misfits(&faceplate, &pairs, "Main.V101", storage);
    assert_eq!(valve.len(), 2, "{valve:?}");
    assert!(valve[0].contains("Running"), "{valve:?}");
    assert!(valve[1].contains("Speed"), "{valve:?}");
    let open = parse_faceplate(
        "hmi/panels/valve-card.toml",
        "format = 1\ntitle = \"Valve\"\nsize = [320, 160]\n\n[fields.Open]\naccepts = [\"number\"]\n",
    )
    .expect("the valve card loads");
    assert_eq!(
        misfits(&open, &[], "Main.V101", storage),
        vec!["Main.V101.Open is not a number for the field Open".to_string()],
        "a member of another kind does not fit"
    );
    assert_eq!(
        misfits(&faceplate, &pairs, "Main.Level", storage),
        vec!["\"Main.Level\" is not an instance of the program".to_string()]
    );
    assert_eq!(
        misfits(&faceplate, &[], "Main.P101", storage).len(),
        1,
        "without the pair, Speed is the member of its own name, which a pump lacks"
    );
}

#[test]
fn the_fitting_instances_are_the_blocks_it_fits_in_declaration_order() {
    let harness = harness();
    let (faceplate, pairs) = card();
    let programs = harness.runtime().programs();
    let is_program = |name: &str| find_ignoring_case(programs, name).is_some();
    assert_eq!(
        fitting_instances(&faceplate, &pairs, harness.runtime(), &is_program, 100),
        ["Main.P101", "Main.North.Feed", "Main.P102"]
    );
}

#[test]
fn declared_field_kinds_use_the_same_string_contract_and_wildcard_as_live_fields() {
    let boolean = HmiField {
        accepts: vec!["boolean".into()],
    };
    assert!(accepts_declared_kinds(
        &boolean,
        &[ValueKind::Boolean, ValueKind::Value]
    ));
    assert!(!accepts_declared_kinds(
        &boolean,
        &[ValueKind::Number, ValueKind::Value]
    ));
    assert!(accepts_declared_kinds(
        &HmiField { accepts: vec![] },
        &[ValueKind::Text]
    ));
}

/// Specification 39, §8.5 and §10.5: a definition's member `{instance}_…` names the variable beside
/// the instance, named after it, as a PID loop written as ordinary code keeps its values; the
/// faceplate fits and binds them, and the Plant library's PID definition loads with them.
#[test]
fn a_faceplate_binds_the_variables_beside_its_instance() {
    let program = r#"
FUNCTION_BLOCK Loop
VAR
    Gain : REAL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    TIC101 : Loop;
    TIC101_Setpoint : LREAL := 45.0;
    TIC101_Manual : BOOL := TRUE;
END_VAR
    TIC101();
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(program).expect("the program compiles");
    harness.cycle();
    let faceplate = parse_faceplate(
        "hmi/panels/loop.toml",
        "format = 1\ntitle = \"Loop\"\nsize = [320, 160]\n\n[fields.Setpoint]\naccepts = [\"number\"]\n\n[fields.Manual]\naccepts = [\"boolean\"]\n\n[[object]]\nid = \"setpoint\"\ntype = \"value\"\nat = [0, 0]\nsize = [100, 40]\n\n[object.bind]\nvalue = \"$Setpoint\"\n",
    )
    .expect("the panel loads");
    let pair = parse_pair(
        "hmi/pairs/loop.toml",
        "format = 1\nblock = \"Loop\"\nfaceplate = \"loop\"\nsymbol = \"controller\"\n\n[fields]\nSetpoint = \"{instance}_Setpoint\"\nManual = \"{instance}_Manual\"\n",
    )
    .expect("a pair naming variables beside the instance loads");
    let pairs = vec![pair];
    assert_eq!(
        misfits(
            &faceplate,
            &pairs,
            "Main.TIC101",
            harness.runtime().storage()
        ),
        Vec::<String>::new(),
        "the variables beside the instance hold the fields"
    );
    let bound = bind_fields(faceplate, "Main.TIC101", &pairs[0].fields);
    assert_eq!(
        bound.popup.page.objects[0].bind["value"].path,
        "Main.TIC101_Setpoint"
    );
    assert_eq!(
        crate::hmi::page_files::pair_member_path("Main.P101", "Running"),
        "Main.P101.Running"
    );
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../libraries/plant/hmi/pairs/plant-pid.toml");
    let text = std::fs::read_to_string(shipped).expect("the PID definition");
    let pid = parse_pair("hmi/pairs/plant-pid.toml", &text).expect("the PID definition loads");
    assert_eq!(pid.block, "Control.PID");
    assert!(pid
        .settings
        .iter()
        .any(|setting| setting.member == "{instance}_Parameters.ProportionalGain"));
}
