use super::*;

fn position(line: u32, character: u32) -> DocumentPosition {
    DocumentPosition { line, character }
}

fn range(start: (u32, u32), end: (u32, u32)) -> DocumentRange {
    DocumentRange {
        start: position(start.0, start.1),
        end: position(end.0, end.1),
    }
}

fn ranged(start: (u32, u32), end: (u32, u32), text: &str) -> ContentChange {
    ContentChange {
        range: Some(range(start, end)),
        text: text.to_string(),
    }
}

fn full(text: &str) -> ContentChange {
    ContentChange {
        range: None,
        text: text.to_string(),
    }
}

#[test]
fn empty_document_has_one_empty_line() {
    let index = LineIndex::new("");
    assert_eq!(index.line_count(), 1);
    assert_eq!(index.line_start(0), Some(0));
    assert_eq!(index.line_text(0), Some(""));
    assert_eq!(index.offset_to_line_col(0), (0, 0));
    assert_eq!(index.position_to_offset(position(0, 0)), Some(0));
}

#[test]
fn trailing_lf_creates_final_empty_line() {
    let index = LineIndex::new("A\n");
    assert_eq!(index.line_count(), 2);
    assert_eq!(index.line_text(0), Some("A"));
    assert_eq!(index.line_text(1), Some(""));
    assert_eq!(index.line_start(1), Some(2));
}

#[test]
fn trailing_crlf_creates_one_final_empty_line() {
    let index = LineIndex::new("A\r\n");
    assert_eq!(index.line_count(), 2);
    assert_eq!(index.line_text(0), Some("A"));
    assert_eq!(index.line_text(1), Some(""));
    assert_eq!(index.line_start(1), Some(3));
}

#[test]
fn trailing_lone_cr_creates_final_empty_line() {
    let index = LineIndex::new("A\r");
    assert_eq!(index.line_count(), 2);
    assert_eq!(index.line_text(0), Some("A"));
    assert_eq!(index.line_text(1), Some(""));
    assert_eq!(index.line_start(1), Some(2));
}

#[test]
fn mixed_line_terminators_are_each_recognized() {
    let index = LineIndex::new("A\nB\r\nC\rD");
    assert_eq!(index.line_count(), 4);
    assert_eq!(index.line_text(0), Some("A"));
    assert_eq!(index.line_text(1), Some("B"));
    assert_eq!(index.line_text(2), Some("C"));
    assert_eq!(index.line_text(3), Some("D"));
}

#[test]
fn line_access_outside_document_returns_none() {
    let index = LineIndex::new("A\nB");
    assert_eq!(index.line_start(2), None);
    assert_eq!(index.line_text(2), None);
}

#[test]
fn offset_inside_lf_maps_to_preceding_line_end() {
    let index = LineIndex::new("AB\nC");
    assert_eq!(index.offset_to_line_col(2), (0, 2));
    assert_eq!(index.offset_to_line_col(3), (1, 0));
}

#[test]
fn both_crlf_bytes_map_to_preceding_line_end() {
    let index = LineIndex::new("AB\r\nC");
    assert_eq!(index.offset_to_line_col(2), (0, 2));
    assert_eq!(index.offset_to_line_col(3), (0, 2));
    assert_eq!(index.offset_to_line_col(4), (1, 0));
}

#[test]
fn lone_cr_maps_to_preceding_line_end() {
    let index = LineIndex::new("AB\rC");
    assert_eq!(index.offset_to_line_col(2), (0, 2));
    assert_eq!(index.offset_to_line_col(3), (1, 0));
}

#[test]
fn offset_beyond_document_clamps_to_eof() {
    let index = LineIndex::new("A\nBC");
    assert_eq!(index.offset_to_line_col(usize::MAX), (1, 2));
}

#[test]
fn offset_inside_multibyte_scalar_moves_to_scalar_start() {
    let index = LineIndex::new("A😀B");
    let emoji_start = "A".len();
    for offset in emoji_start..emoji_start + '😀'.len_utf8() {
        assert_eq!(index.offset_to_line_col(offset), (0, 1), "{offset}");
    }
}

#[test]
fn supplementary_scalar_counts_as_two_utf16_units() {
    let index = LineIndex::new("A😀B");
    assert_eq!(index.offset_to_line_col(0), (0, 0));
    assert_eq!(index.offset_to_line_col(1), (0, 1));
    assert_eq!(index.offset_to_line_col(5), (0, 3));
    assert_eq!(index.offset_to_line_col(6), (0, 4));
}

#[test]
fn bmp_multibyte_scalar_counts_as_one_utf16_unit() {
    let index = LineIndex::new("AéB");
    assert_eq!(index.offset_to_line_col(1), (0, 1));
    assert_eq!(index.offset_to_line_col(3), (0, 2));
    assert_eq!(index.offset_to_line_col(4), (0, 3));
}

#[test]
fn valid_scalar_boundary_positions_round_trip() {
    let source = "A😀é\n中Z";
    let index = LineIndex::new(source);
    for offset in (0..=source.len()).filter(|offset| source.is_char_boundary(*offset)) {
        if matches!(source.as_bytes().get(offset), Some(b'\n')) {
            continue;
        }
        let (line, character) = index.offset_to_line_col(offset);
        assert_eq!(
            index.position_to_offset(position(line, character)),
            Some(offset),
            "offset {offset}"
        );
    }
}

#[test]
fn position_inside_supplementary_scalar_selects_scalar_start() {
    let index = LineIndex::new("😀X");
    assert_eq!(index.position_to_offset(position(0, 0)), Some(0));
    assert_eq!(index.position_to_offset(position(0, 1)), Some(0));
    assert_eq!(index.position_to_offset(position(0, 2)), Some(4));
}

#[test]
fn position_beyond_line_defaults_to_line_end() {
    let index = LineIndex::new("AB\nC");
    assert_eq!(index.position_to_offset(position(0, 99)), Some(2));
    assert_eq!(index.position_to_offset(position(1, 99)), Some(4));
}

#[test]
fn position_on_out_of_range_line_is_rejected() {
    let index = LineIndex::new("AB\nC");
    assert_eq!(index.position_to_offset(position(2, 0)), None);
    assert_eq!(index.position_to_offset(position(u32::MAX, 0)), None);
}

#[test]
fn utf16_length_counts_ascii_bmp_and_supplementary_scalars() {
    let source = "Aé😀Z";
    let index = LineIndex::new(source);
    assert_eq!(index.utf16_len_between(0, source.len()), 5);
}

#[test]
fn utf16_length_includes_line_terminator_units() {
    let index = LineIndex::new("A\r\nB");
    assert_eq!(index.utf16_len_between(0, 4), 4);
}

#[test]
fn utf16_length_clamps_offsets_to_scalar_boundaries() {
    let source = "A😀B";
    let index = LineIndex::new(source);
    assert_eq!(index.utf16_len_between(2, 5), 2);
    assert_eq!(index.utf16_len_between(2, 3), 0);
}

#[test]
fn utf16_length_returns_zero_for_empty_or_reversed_interval() {
    let index = LineIndex::new("ABC");
    assert_eq!(index.utf16_len_between(1, 1), 0);
    assert_eq!(index.utf16_len_between(2, 1), 0);
}

#[test]
fn incremental_change_inserts_at_start() {
    let updated = apply_content_changes("ABC", &[ranged((0, 0), (0, 0), "X")]).unwrap();
    assert_eq!(updated, "XABC");
}

#[test]
fn incremental_change_inserts_at_line_end() {
    let updated = apply_content_changes("AB\nC", &[ranged((0, 2), (0, 2), "X")]).unwrap();
    assert_eq!(updated, "ABX\nC");
}

#[test]
fn incremental_change_deletes_range() {
    let updated = apply_content_changes("ABCDE", &[ranged((0, 1), (0, 4), "")]).unwrap();
    assert_eq!(updated, "AE");
}

#[test]
fn incremental_change_replaces_single_line_range() {
    let updated = apply_content_changes("value := 1;", &[ranged((0, 9), (0, 10), "42")]).unwrap();
    assert_eq!(updated, "value := 42;");
}

#[test]
fn incremental_change_replaces_multiline_range() {
    let updated = apply_content_changes("one\ntwo\nthree", &[ranged((0, 1), (2, 2), "X")]).unwrap();
    assert_eq!(updated, "oXree");
}

#[test]
fn incremental_change_uses_utf16_columns() {
    let updated = apply_content_changes("😀x", &[ranged((0, 2), (0, 3), "y")]).unwrap();
    assert_eq!(updated, "😀y");
}

#[test]
fn incremental_change_inside_surrogate_pair_uses_scalar_start() {
    let updated = apply_content_changes("😀x", &[ranged((0, 1), (0, 2), "A")]).unwrap();
    assert_eq!(updated, "Ax");
}

#[test]
fn incremental_column_beyond_line_defaults_to_line_end() {
    let updated = apply_content_changes("AB\nC", &[ranged((0, 99), (0, 100), "X")]).unwrap();
    assert_eq!(updated, "ABX\nC");
}

#[test]
fn sequential_ranges_use_text_from_preceding_change() {
    let updated = apply_content_changes(
        "ABC",
        &[ranged((0, 1), (0, 2), "123"), ranged((0, 4), (0, 5), "Z")],
    )
    .unwrap();
    assert_eq!(updated, "A123Z");
}

#[test]
fn full_change_replaces_complete_document() {
    let updated = apply_content_changes("old", &[full("new\ntext")]).unwrap();
    assert_eq!(updated, "new\ntext");
}

#[test]
fn full_change_can_be_followed_by_incremental_change() {
    let updated =
        apply_content_changes("old", &[full("😀x\n"), ranged((0, 2), (0, 3), "y")]).unwrap();
    assert_eq!(updated, "😀y\n");
}

#[test]
fn incremental_change_can_be_followed_by_full_change() {
    let updated =
        apply_content_changes("old", &[ranged((0, 0), (0, 1), "X"), full("final")]).unwrap();
    assert_eq!(updated, "final");
}

#[test]
fn replacement_preserves_untouched_crlf_bytes() {
    let updated =
        apply_content_changes("one\r\ntwo\r\nthree", &[ranged((1, 0), (1, 3), "TWO")]).unwrap();
    assert_eq!(updated, "one\r\nTWO\r\nthree");
}

#[test]
fn out_of_range_start_line_has_typed_stable_error() {
    let error = apply_content_changes("A\n", &[ranged((2, 0), (2, 0), "X")]).unwrap_err();
    assert_eq!(
        error,
        ContentChangeError::LineOutOfBounds {
            endpoint: "start",
            line: 2,
            line_count: 2,
        }
    );
    assert_eq!(
        error.to_string(),
        "change start line 2 is outside the 2-line document"
    );
}

#[test]
fn out_of_range_end_line_has_typed_stable_error() {
    let error = apply_content_changes("A\n", &[ranged((0, 0), (2, 0), "X")]).unwrap_err();
    assert_eq!(
        error,
        ContentChangeError::LineOutOfBounds {
            endpoint: "end",
            line: 2,
            line_count: 2,
        }
    );
    assert_eq!(
        error.to_string(),
        "change end line 2 is outside the 2-line document"
    );
}

#[test]
fn reversed_same_line_range_is_rejected() {
    let error = apply_content_changes("ABC", &[ranged((0, 2), (0, 1), "X")]).unwrap_err();
    assert_eq!(error, ContentChangeError::ReversedRange);
    assert_eq!(error.to_string(), "change range start is after its end");
}

#[test]
fn reversed_multiline_range_is_rejected() {
    let error = apply_content_changes("A\nB", &[ranged((1, 0), (0, 1), "X")]).unwrap_err();
    assert_eq!(error, ContentChangeError::ReversedRange);
}

#[test]
fn invalid_later_change_returns_no_partially_updated_document() {
    let original = "ABC";
    let result = apply_content_changes(
        original,
        &[ranged((0, 0), (0, 1), "X"), ranged((9, 0), (9, 0), "Y")],
    );
    assert_eq!(
        result,
        Err(ContentChangeError::LineOutOfBounds {
            endpoint: "start",
            line: 9,
            line_count: 1,
        })
    );
    assert_eq!(original, "ABC");
}

#[test]
fn empty_change_batch_preserves_document_exactly() {
    let original = "A\r\n😀\n";
    assert_eq!(apply_content_changes(original, &[]).unwrap(), original);
}
