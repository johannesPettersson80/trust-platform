use trust_ide::rename::TextEdit;

use super::*;

fn edit(start: u32, end: u32, replacement: &str) -> TextEdit {
    TextEdit {
        range: TextRange::new(TextSize::from(start), TextSize::from(end)),
        new_text: replacement.to_string(),
    }
}

fn completion(label: &str, kind: &str, priority: u32) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: kind.to_string(),
        detail: None,
        documentation: None,
        insert_text: None,
        text_edit: None,
        sort_priority: priority,
    }
}

#[test]
fn position_to_offset_counts_unicode_scalars_not_bytes() {
    let text = "åb\n日本";
    assert_eq!(
        position_to_text_size(
            text,
            &Position {
                line: 0,
                character: 1,
            }
        ),
        TextSize::from(2)
    );
    assert_eq!(
        position_to_text_size(
            text,
            &Position {
                line: 1,
                character: 1,
            }
        ),
        TextSize::from(7)
    );
}

#[test]
fn position_at_line_start_maps_after_previous_newline() {
    assert_eq!(
        position_to_text_size(
            "abc\ndef",
            &Position {
                line: 1,
                character: 0,
            }
        ),
        TextSize::from(4)
    );
}

#[test]
fn character_beyond_line_clamps_to_that_line_end() {
    assert_eq!(
        position_to_text_size(
            "abc\ndef",
            &Position {
                line: 0,
                character: 99,
            }
        ),
        TextSize::from(3)
    );
}

#[test]
fn line_beyond_document_clamps_to_document_end() {
    assert_eq!(
        position_to_text_size(
            "abc\ndef",
            &Position {
                line: 99,
                character: 0,
            }
        ),
        TextSize::from(7)
    );
}

#[test]
fn offset_to_position_counts_lines_and_unicode_scalars() {
    let first = text_offset_to_position("åb\n日本", TextSize::from(2));
    assert_eq!((first.line, first.character), (0, 1));
    let second = text_offset_to_position("åb\n日本", TextSize::from(7));
    assert_eq!((second.line, second.character), (1, 1));
}

#[test]
fn text_range_projection_preserves_both_boundaries() {
    let range = text_range_to_ide_range(
        "one\ntwo",
        TextRange::new(TextSize::from(1), TextSize::from(6)),
    );
    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 1);
    assert_eq!(range.end.line, 1);
    assert_eq!(range.end.character, 2);
}

#[test]
fn rename_edits_apply_from_highest_offset_to_lowest() {
    let output = apply_text_edits(
        "alpha beta alpha",
        &[edit(0, 5, "x"), edit(11, 16, "long_name")],
    )
    .expect("rename edits");
    assert_eq!(output, "x beta long_name");
}

#[test]
fn insertion_and_deletion_edits_are_supported() {
    let output = apply_text_edits("abc", &[edit(0, 0, "x"), edit(1, 2, "")]).expect("edits");
    assert_eq!(output, "xac");
}

#[test]
fn text_range_constructor_rejects_reversed_range_before_edit_application() {
    assert!(std::panic::catch_unwind(|| edit(2, 1, "x")).is_err());
}

#[test]
fn out_of_bounds_rename_range_is_rejected() {
    let error = apply_text_edits("abc", &[edit(1, 4, "x")]).expect_err("range");
    assert_eq!(error.kind(), IdeErrorKind::InvalidInput);
}

#[test]
fn overlapping_rename_ranges_are_rejected() {
    let error = apply_text_edits("abcdef", &[edit(0, 4, "x"), edit(2, 5, "y")])
        .expect_err("overlapping ranges");
    assert_eq!(error.kind(), IdeErrorKind::InvalidInput);
}

#[test]
fn completion_prefix_uses_current_line_and_identifier_characters() {
    let text = "PROGRAM Main\nmotor_sp";
    assert_eq!(
        completion_prefix(
            text,
            Position {
                line: 1,
                character: 8,
            }
        ),
        "motor_sp"
    );
}

#[test]
fn completion_prefix_stops_at_operator_or_whitespace() {
    assert_eq!(
        completion_prefix(
            "x := motor",
            Position {
                line: 0,
                character: 10,
            }
        ),
        "motor"
    );
    assert_eq!(
        completion_prefix(
            "obj.member",
            Position {
                line: 0,
                character: 10,
            }
        ),
        "member"
    );
}

#[test]
fn completion_prefix_clamps_cursor_past_line_end() {
    assert_eq!(
        completion_prefix(
            "motor",
            Position {
                line: 0,
                character: 99,
            }
        ),
        "motor"
    );
}

#[test]
fn identifier_contract_is_ascii_and_nonempty() {
    for valid in ["x", "_x", "Motor_17", "A0"] {
        assert!(is_identifier(valid), "{valid}");
    }
    for invalid in ["", "0x", "a-b", "a b", "møtor"] {
        assert!(!is_identifier(invalid), "{invalid}");
    }
}

#[test]
fn in_scope_symbol_extraction_covers_pous_types_and_variables() {
    let symbols = extract_in_scope_symbols(
        "PROGRAM Main\n\
         FUNCTION_BLOCK Motor\n\
         TYPE State : INT;\n\
         CLASS Controller\n\
         speed, target : REAL;\n\
         enabled : BOOL;\n",
    );
    for expected in [
        "Main",
        "Motor",
        "State",
        "Controller",
        "speed",
        "target",
        "enabled",
    ] {
        assert!(symbols.contains(expected), "{expected}");
    }
}

#[test]
fn in_scope_symbol_extraction_ignores_comment_only_lines_and_invalid_names() {
    let symbols = extract_in_scope_symbols(
        "// hidden : INT;\n\
         (* blocked : INT; *)\n\
         9bad : INT;\n\
         good : INT;\n",
    );
    assert_eq!(symbols, BTreeSet::from(["good".to_string()]));
}

#[test]
fn completion_ranking_prefers_symbols_then_keywords_then_nonprefix() {
    assert_eq!(completion_rank(&completion("motor", "symbol", 0), "mo"), 0);
    assert_eq!(completion_rank(&completion("MOD", "keyword", 0), "mo"), 1);
    assert_eq!(completion_rank(&completion("other", "symbol", 0), "mo"), 2);
}

#[test]
fn completion_contract_adds_in_scope_prefix_matches_and_deduplicates_case() {
    let text = "PROGRAM Main\nVAR\n  MotorSpeed : REAL;\nEND_VAR\nMot";
    let mut items = vec![
        completion("MOTOR_SPEED", "variable", 5),
        completion("MOD", "keyword", 0),
        completion("motor_speed", "variable", 1),
    ];
    apply_completion_relevance_contract(
        &mut items,
        text,
        Position {
            line: 4,
            character: 3,
        },
        None,
    );

    assert_eq!(items[0].label, "MotorSpeed");
    assert_eq!(
        items
            .iter()
            .filter(|item| item.label.eq_ignore_ascii_case("motor_speed"))
            .count(),
        1
    );
}

#[test]
fn completion_contract_applies_limit_after_ranking_and_deduplication() {
    let mut items = vec![
        completion("zeta", "symbol", 0),
        completion("alpha", "symbol", 0),
        completion("ALPHA", "symbol", 1),
    ];
    apply_completion_relevance_contract(
        &mut items,
        "a",
        Position {
            line: 0,
            character: 1,
        },
        Some(1),
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label.to_ascii_lowercase(), "alpha");
}

#[test]
fn empty_completion_prefix_preserves_order_and_only_applies_limit() {
    let mut items = vec![
        completion("zeta", "symbol", 0),
        completion("alpha", "symbol", 0),
    ];
    apply_completion_relevance_contract(
        &mut items,
        "",
        Position {
            line: 0,
            character: 0,
        },
        Some(1),
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "zeta");
}
