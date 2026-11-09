//! Packed structures (`{attribute 'pack_mode' := 'n'}`, specification 02 §3.5.2): the addresses a
//! pack mode gives a structure's members, from each member's size and alignment. `SIZEOF` and the
//! layout a packed structure takes in an overlap's backing and on the wire share this one rule.

/// The pack mode the attribute's value names: `0`, `1`, `2`, `4` or `8`.
#[must_use]
pub fn parse_pack_mode(value: &str) -> Option<u8> {
    match value.trim() {
        "0" => Some(0),
        "1" => Some(1),
        "2" => Some(2),
        "4" => Some(4),
        "8" => Some(8),
        _ => None,
    }
}

/// The largest alignment a member of a structure packed by `mode` keeps: 1 for `0` and `1`, the
/// mode otherwise.
#[must_use]
pub fn pack_alignment(mode: u8) -> u64 {
    u64::from(mode.max(1))
}

/// A structure's layout by its pack mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedLayout {
    /// Each member's offset, in declaration order.
    pub offsets: Vec<u64>,
    /// The end of the last member, rounded up to the alignment.
    pub size: u64,
    /// The largest alignment a member lies at.
    pub align: u64,
}

/// The layout of members of the given `(size, alignment)`, in order, packed by `mode`: each at the
/// first address after the member before it that the smaller of its alignment and the mode's
/// divides, the whole rounded up to the largest such alignment (specification 02 §3.5.2). `None`
/// when an address overflows.
#[must_use]
pub fn packed_layout(members: &[(u64, u64)], mode: u8) -> Option<PackedLayout> {
    let cap = pack_alignment(mode);
    let mut offsets = Vec::with_capacity(members.len());
    let mut end = 0u64;
    let mut align = 1u64;
    for &(size, member_align) in members {
        let at = member_align.clamp(1, cap);
        let offset = end.checked_next_multiple_of(at)?;
        offsets.push(offset);
        end = offset.checked_add(size)?;
        align = align.max(at);
    }
    Some(PackedLayout {
        offsets,
        size: end.checked_next_multiple_of(align)?,
        align,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CODESYS's first example, `BOOL, INT, BOOL, BOOL`: with `1` at 0, 1, 3 and 4 (5 bytes); with
    /// `2` at 0, 2, 4 and 5, rounded up to 6. Catches a member placed at its natural alignment under
    /// `1` (the INT at 2) and a size left unrounded under `2` (5).
    #[test]
    fn the_first_vendor_example_packs_as_the_help_lays_it_out() {
        let members = [(1, 1), (2, 2), (1, 1), (1, 1)];
        let one = packed_layout(&members, 1).unwrap();
        assert_eq!((one.offsets, one.size, one.align), (vec![0, 1, 3, 4], 5, 1));
        let two = packed_layout(&members, 2).unwrap();
        assert_eq!((two.offsets, two.size, two.align), (vec![0, 2, 4, 5], 6, 2));
    }

    /// CODESYS's second example, `BOOL, BYTE, WORD, BYTE, DWORD, BYTE, BYTE, DWORD`: 0, 1, 2, 4, 5,
    /// 9, 10, 11 (15 bytes) with `0` and `1`; 0, 1, 2, 4, 6, 10, 11, 12 (16) with `2`; 0, 1, 2, 4,
    /// 8, 12, 13, 16 (20) with `4` and `8`. Catches `0` read as no packing, a DWORD under `2` at 8,
    /// and `8` aligning past a member's own alignment.
    #[test]
    fn the_second_vendor_example_packs_as_the_help_lays_it_out() {
        let members = [
            (1, 1),
            (1, 1),
            (2, 2),
            (1, 1),
            (4, 4),
            (1, 1),
            (1, 1),
            (4, 4),
        ];
        for (mode, offsets, size) in [
            (0, vec![0, 1, 2, 4, 5, 9, 10, 11], 15),
            (1, vec![0, 1, 2, 4, 5, 9, 10, 11], 15),
            (2, vec![0, 1, 2, 4, 6, 10, 11, 12], 16),
            (4, vec![0, 1, 2, 4, 8, 12, 13, 16], 20),
            (8, vec![0, 1, 2, 4, 8, 12, 13, 16], 20),
        ] {
            let layout = packed_layout(&members, mode).unwrap();
            assert_eq!((layout.offsets, layout.size), (offsets, size), "{mode}");
        }
    }

    /// Only the five pack modes CODESYS names are pack modes.
    #[test]
    fn a_pack_mode_is_one_of_the_five_the_vendor_names() {
        for (text, mode) in [("0", 0), ("1", 1), ("2", 2), ("4", 4), (" 8 ", 8)] {
            assert_eq!(parse_pack_mode(text), Some(mode), "{text}");
        }
        for text in ["3", "16", "-1", "", "x"] {
            assert_eq!(parse_pack_mode(text), None, "{text}");
        }
    }
}
