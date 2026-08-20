use super::*;

#[test]
fn line_start_table_always_begins_at_zero() {
    assert_eq!(line_starts(""), vec![0]);
    assert_eq!(line_starts("abc"), vec![0]);
}

#[test]
fn line_start_table_adds_byte_after_every_lf_including_final_lf() {
    assert_eq!(line_starts("a\nbc\n"), vec![0, 2, 5]);
}

#[test]
fn crlf_keeps_carriage_return_in_preceding_line() {
    assert_eq!(line_starts("a\r\nb"), vec![0, 3]);
    assert_eq!(offset_to_line_col("a\r\nb", 2), (0, 2));
    assert_eq!(offset_to_line_col("a\r\nb", 3), (1, 0));
}

#[test]
fn offset_projection_handles_line_starts_ends_and_newline_byte() {
    let source = "ab\ncd";
    assert_eq!(offset_to_line_col(source, 0), (0, 0));
    assert_eq!(offset_to_line_col(source, 2), (0, 2));
    assert_eq!(offset_to_line_col(source, 3), (1, 0));
    assert_eq!(offset_to_line_col(source, 5), (1, 2));
}

#[test]
fn offset_beyond_source_clamps_to_source_end() {
    assert_eq!(offset_to_line_col("ab\ncd", 500), (1, 2));
}

#[test]
fn location_projection_uses_start_offset_only() {
    let location = SourceLocation::new(7, 3, u32::MAX);
    assert_eq!(location_to_line_col("ab\ncd", &location), (1, 0));
}

#[test]
fn breakpoint_resolution_rejects_line_beyond_source() {
    let source = "one\ntwo";
    let statements = [SourceLocation::new(1, 0, 3)];
    assert_eq!(
        resolve_breakpoint_location(source, 1, &statements, 2, 0),
        None
    );
}

#[test]
fn breakpoint_resolution_ignores_other_file_ids() {
    let source = "one\ntwo";
    let statements = [SourceLocation::new(2, 0, 3), SourceLocation::new(2, 4, 7)];
    assert_eq!(
        resolve_breakpoint_location(source, 1, &statements, 0, 0),
        None
    );
}

#[test]
fn breakpoint_resolution_prefers_greatest_start_not_after_column() {
    let source = "  first   second\n";
    let first = SourceLocation::new(1, 2, 7);
    let second = SourceLocation::new(1, 10, 16);
    let statements = [first, second];
    assert_eq!(
        resolve_breakpoint_location(source, 1, &statements, 0, 12),
        Some(second)
    );
    assert_eq!(
        resolve_breakpoint_location(source, 1, &statements, 0, 8),
        Some(first)
    );
}

#[test]
fn breakpoint_resolution_uses_smallest_later_start_when_none_precedes() {
    let source = "  first   second\n";
    let first = SourceLocation::new(1, 2, 7);
    let second = SourceLocation::new(1, 10, 16);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[second, first], 0, 0),
        Some(first)
    );
}

#[test]
fn breakpoint_resolution_clamps_column_to_line_end() {
    let source = "first\nsecond\n";
    let first = SourceLocation::new(1, 0, 5);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[first], 0, u32::MAX),
        Some(first)
    );
}

#[test]
fn same_line_candidate_wins_over_cross_line_containing_span() {
    let source = "IF x THEN\n  y := 1;\nEND_IF\n";
    let outer = SourceLocation::new(1, 0, source.len() as u32);
    let inner_start = source.find("y := 1").expect("inner") as u32;
    let inner = SourceLocation::new(1, inner_start, inner_start + 7);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[outer, inner], 1, 0),
        Some(inner)
    );
}

#[test]
fn narrowest_containing_statement_is_selected_without_line_start_candidate() {
    let source = "outer\nmiddle\nend";
    let outer = SourceLocation::new(1, 0, source.len() as u32);
    let narrower = SourceLocation::new(1, 2, 11);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[outer, narrower], 1, 2),
        Some(narrower)
    );
}

#[test]
fn containment_includes_runtime_location_end_offset() {
    let source = "outer\nmiddle\nend";
    let containing = SourceLocation::new(1, 0, 8);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[containing], 1, 2),
        Some(containing)
    );
}

#[test]
fn earliest_later_statement_is_selected_when_line_and_containment_miss() {
    let source = "request\nnone\nlater_a\nlater_b\n";
    let later_a = source.find("later_a").expect("a") as u32;
    let later_b = source.find("later_b").expect("b") as u32;
    let statements = [
        SourceLocation::new(1, later_b, later_b + 7),
        SourceLocation::new(1, later_a, later_a + 7),
    ];
    assert_eq!(
        resolve_breakpoint_location(source, 1, &statements, 1, 0),
        Some(SourceLocation::new(1, later_a, later_a + 7))
    );
}

#[test]
fn no_earlier_statement_is_selected_as_global_fallback() {
    let source = "earlier\nrequest\n";
    let statement = SourceLocation::new(1, 0, 7);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[statement], 1, 0),
        None
    );
}

#[test]
fn empty_statement_set_returns_none_on_valid_line() {
    assert_eq!(resolve_breakpoint_location("line", 1, &[], 0, 0), None);
}

#[test]
fn final_empty_line_is_a_valid_request_line() {
    let source = "first\n";
    let statement = SourceLocation::new(1, 0, 5);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[statement], 1, 0),
        None
    );
}

#[test]
fn duplicate_statement_start_keeps_registration_order() {
    let source = "statement";
    let first = SourceLocation::new(1, 0, 5);
    let second = SourceLocation::new(1, 0, 9);
    assert_eq!(
        resolve_breakpoint_location(source, 1, &[first, second], 0, 0),
        Some(first)
    );
}
