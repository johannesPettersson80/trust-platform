//! Specification 39, §8.5 and §8.7: faceplate files (a popup with a description and the fields it
//! needs, its objects binding fields as `$Name`) and pair files (a block type, its faceplate and
//! symbol, the members of its fields and the rules for adding it to a program), each written back
//! canonically and checked at their lines.

use std::collections::BTreeMap;

use super::*;

const FACEPLATE_FILE: &str = "hmi/panels/pump.toml";
const PAIR_FILE: &str = "hmi/pairs/pump.toml";

const FACEPLATE: &str = r#"format = 1
title = "Pump"
description = "Transfer pump, fixed speed"
size = [480, 600]
pinnable = true

[fields.Running]
accepts = ["boolean"]

[fields.Starts]
accepts = ["number"]

[[object]]
id = "symbol"
type = "pump"
at = [24, 64]
size = [128, 108]

[object.bind]
running = "$Running"

[[object]]
id = "starts"
type = "value"
at = [24, 200]
size = [160, 48]

[object.bind]
value = "$Starts"
"#;

const PAIR: &str = r#"format = 1
block = "Equipment.Pump"
faceplate = "pump"
symbol = "pump"
variants = ["variable-speed"]

[fields]
Running = "IsRunning"

[program]
position = "actuators"
cycle = [10, 1000]

[program.initial]
Mode = 1
"#;

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn a_faceplate_keeps_its_description_and_fields_and_is_written_back_byte_for_byte() {
    let faceplate = parse_faceplate(FACEPLATE_FILE, FACEPLATE).expect("the faceplate loads");
    assert_eq!(
        faceplate.description.as_deref(),
        Some("Transfer pump, fixed speed")
    );
    assert_eq!(
        faceplate.fields.keys().collect::<Vec<_>>(),
        ["Running", "Starts"]
    );
    assert_eq!(faceplate.fields["Running"].accepts, ["boolean"]);
    assert_eq!(
        faceplate.popup.page.objects[0].bind["running"].path,
        "$Running"
    );
    assert!(faceplate.popup.pinnable);
    assert_eq!(write_faceplate(&faceplate), FACEPLATE);
}

#[test]
fn a_faceplate_binds_only_its_fields_and_names_only_kinds_of_value() {
    let undeclared = FACEPLATE.replace("value = \"$Starts\"", "value = \"$Speed\"");
    let errors = parse_faceplate(FACEPLATE_FILE, &undeclared).expect_err("an undeclared field");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&undeclared, "value = \"$Speed\""));
    assert!(
        errors[0]
            .message
            .contains("`$Speed` is not a field of this faceplate"),
        "{}",
        errors[0].message
    );
    let kind = FACEPLATE.replace("accepts = [\"number\"]", "accepts = [\"colour\"]");
    let errors = parse_faceplate(FACEPLATE_FILE, &kind).expect_err("an unknown kind");
    assert_eq!(errors[0].line, line_of(&kind, "accepts = [\"colour\"]"));
    assert!(
        errors[0]
            .message
            .contains("`colour` is not a kind of value"),
        "{}",
        errors[0].message
    );
}

#[test]
fn a_pair_names_its_block_faceplate_symbol_members_and_program_rules_and_is_written_back_byte_for_byte(
) {
    let pair = parse_pair(PAIR_FILE, PAIR).expect("the pair loads");
    assert_eq!(pair.block, "Equipment.Pump");
    assert_eq!(pair.faceplate, "pump");
    assert_eq!(pair.symbol, "pump");
    assert_eq!(pair.variants, ["variable-speed"]);
    assert_eq!(
        pair.fields,
        BTreeMap::from([("Running".to_string(), "IsRunning".to_string())])
    );
    let program = pair.program.as_ref().expect("its program rules");
    assert_eq!(program.position, Some(HmiCallPosition::Actuators));
    assert_eq!(program.cycle, Some([10, 1000]));
    assert_eq!(program.initial.get("Mode"), Some(&HmiScalar::Number(1.0)));
    assert_eq!(write_pair(&pair), PAIR);
}

#[test]
fn a_pairs_symbol_bindings_are_read_and_written_back_byte_for_byte() {
    // §8.5: `[bind]` names the member each of the symbol's properties shows.
    let text = "format = 1\nblock = \"Equipment.Tank\"\nfaceplate = \"tank\"\nsymbol = \"tank\"\n\n[bind]\nlevel = \"Percent\"\n";
    let pair = parse_pair(PAIR_FILE, text).expect("the pair loads");
    assert_eq!(
        pair.bind,
        BTreeMap::from([("level".to_string(), "Percent".to_string())])
    );
    assert_eq!(write_pair(&pair), text);
}

#[test]
fn a_pair_with_a_position_or_a_cycle_out_of_bounds_is_an_error_at_its_line() {
    let position = PAIR.replace("position = \"actuators\"", "position = \"first\"");
    let errors = parse_pair(PAIR_FILE, &position).expect_err("an unknown position");
    assert_eq!(errors[0].line, line_of(&position, "position = \"first\""));
    assert!(
        errors[0]
            .message
            .contains("`measurements`, `controllers` or `actuators`"),
        "{}",
        errors[0].message
    );
    let cycle = PAIR.replace("cycle = [10, 1000]", "cycle = [1000, 10]");
    let errors = parse_pair(PAIR_FILE, &cycle).expect_err("a cycle upside down");
    assert_eq!(errors[0].line, line_of(&cycle, "cycle = [1000, 10]"));
    assert!(
        errors[0]
            .message
            .contains("the shortest cycle is longer than the longest"),
        "{}",
        errors[0].message
    );
}
