//! The variables a standard function block's native code reads and writes, and their places among
//! its POU's own declarations (checklist P1-4; specification 34 PERF-05).
//!
//! Preparation finds each member by its name among the block's own declarations once, and where its
//! bytes lie in an instance, so a scan reaches a persistent instance's member at its place and
//! compares no names. A place is the one the name lookup finds, so a block reads and writes the same
//! variables either way.

use super::state::{
    STATE_ACTIVE, STATE_LAST_TIME, STATE_PREV_CD, STATE_PREV_CU, STATE_PREV_IN, STATE_TIMING,
    STATE_TRIG_M,
};
use crate::memory::layout::ByteRange;

/// A variable a standard block's native code reads or writes: an input, an output or a state slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Member {
    In,
    Pt,
    Q,
    Et,
    Clk,
    Cu,
    Cd,
    R,
    Ld,
    Pv,
    Qu,
    Qd,
    Cv,
    S,
    R1,
    S1,
    Q1,
    PrevCu,
    PrevCd,
    TrigM,
    LastTime,
    PrevIn,
    Timing,
    Active,
}

/// How many members there are.
pub(crate) const MEMBER_COUNT: usize = 24;

impl Member {
    /// Every member, each at its own index in [`MemberOrdinals`].
    pub(crate) const ALL: [Member; MEMBER_COUNT] = [
        Member::In,
        Member::Pt,
        Member::Q,
        Member::Et,
        Member::Clk,
        Member::Cu,
        Member::Cd,
        Member::R,
        Member::Ld,
        Member::Pv,
        Member::Qu,
        Member::Qd,
        Member::Cv,
        Member::S,
        Member::R1,
        Member::S1,
        Member::Q1,
        Member::PrevCu,
        Member::PrevCd,
        Member::TrigM,
        Member::LastTime,
        Member::PrevIn,
        Member::Timing,
        Member::Active,
    ];

    /// The name the block declares the member under.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Member::In => "IN",
            Member::Pt => "PT",
            Member::Q => "Q",
            Member::Et => "ET",
            Member::Clk => "CLK",
            Member::Cu => "CU",
            Member::Cd => "CD",
            Member::R => "R",
            Member::Ld => "LD",
            Member::Pv => "PV",
            Member::Qu => "QU",
            Member::Qd => "QD",
            Member::Cv => "CV",
            Member::S => "S",
            Member::R1 => "R1",
            Member::S1 => "S1",
            Member::Q1 => "Q1",
            Member::PrevCu => STATE_PREV_CU,
            Member::PrevCd => STATE_PREV_CD,
            Member::TrigM => STATE_TRIG_M,
            Member::LastTime => STATE_LAST_TIME,
            Member::PrevIn => STATE_PREV_IN,
            Member::Timing => STATE_TIMING,
            Member::Active => STATE_ACTIVE,
        }
    }
}

/// The position of each member among a block's own declarations, at the member's index, or
/// [`NO_MEMBER`] for a member the block does not declare.
pub(crate) type MemberOrdinals = [u16; MEMBER_COUNT];

/// The position of a member the block does not declare.
pub(crate) const NO_MEMBER: u16 = u16::MAX;

/// Where a member lies in its instance's own bytes (checklist P1-4): its declared type, its offset
/// from the instance's first byte, its bytes, and whether a presence byte follows its value, as a
/// native-state slot keeps one (specification 34 MEM-12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemberPlace {
    pub(crate) type_id: u32,
    pub(crate) offset: u16,
    pub(crate) bytes: u8,
    pub(crate) presence: bool,
}

/// The place of each member at the member's index, or `None` for a member the block does not
/// declare or whose declaration has no place.
pub(crate) type MemberPlaces = [Option<MemberPlace>; MEMBER_COUNT];

/// A declaration's place as preparation finds it: its offset among the instance's own bytes, its
/// bytes, its type and whether it keeps a presence byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeclaredPlace {
    pub(crate) offset: u32,
    pub(crate) bytes: u32,
    pub(crate) type_id: u32,
    pub(crate) presence: bool,
}

/// The places of the members among `declared`, a block's own declarations in order, each with its
/// name and its place, or none where preparation found none: each member at its first declaration,
/// as [`member_ordinals`] finds it. `None` when a declared member's offset or bytes exceed a
/// [`MemberPlace`]; the members are then found by their names.
pub(crate) fn member_places<'n>(
    declared: impl Iterator<Item = (Option<&'n str>, Option<DeclaredPlace>)> + Clone,
) -> Option<MemberPlaces> {
    let ordinals = member_ordinals(declared.clone().map(|(name, _)| name));
    let mut places = [None; MEMBER_COUNT];
    for (slot, ordinal) in places.iter_mut().zip(ordinals) {
        if ordinal == NO_MEMBER {
            continue;
        }
        if let Some((_, Some(place))) = declared.clone().nth(usize::from(ordinal)) {
            *slot = Some(MemberPlace {
                type_id: place.type_id,
                offset: u16::try_from(place.offset).ok()?,
                bytes: u8::try_from(place.bytes).ok()?,
                presence: place.presence,
            });
        }
    }
    Some(places)
}

/// A persistent instance's own bytes and the places of its block's members in them (checklist
/// P1-4): the instance is found once a call, and each member read or written where it lies.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockPlaces<'a> {
    pub(crate) bytes: ByteRange,
    pub(crate) members: &'a MemberPlaces,
}

impl BlockPlaces<'_> {
    /// The bytes of `member` in the instance, its declared type and the offset of its presence byte,
    /// as the store finds the instance's variable of the member's name; `None` for a member the
    /// block does not declare, or whose place lies past the store's offsets.
    #[inline]
    pub(crate) fn member(self, member: Member) -> Option<(ByteRange, u32, Option<u32>)> {
        let place = self.members[member as usize]?;
        let start = self.bytes.start.checked_add(u32::from(place.offset))?;
        let bytes = u32::from(place.bytes);
        let presence = if place.presence {
            start.checked_add(bytes)
        } else {
            None
        };
        Some((
            ByteRange {
                region: self.bytes.region,
                start,
                bytes,
            },
            place.type_id,
            presence,
        ))
    }
}

/// The most formals a standard block declares: CTUD's CU, CD, R, LD, PV, QU, QD and CV.
pub(crate) const MAX_BUILTIN_FORMALS: usize = 8;

/// The position of each of a block's formals among its own declarations, at the formal's index, or
/// [`NO_MEMBER`] for a formal no declaration names and for an index past the formals.
pub(crate) type FormalPositions = [u16; MAX_BUILTIN_FORMALS];

/// The positions of the members among `declared`, a block's own declaration names in order: each
/// member at the first declaration whose name is the member's name, as the name lookup finds it.
pub(crate) fn member_ordinals<'n>(
    declared: impl Iterator<Item = Option<&'n str>> + Clone,
) -> MemberOrdinals {
    let mut ordinals = [NO_MEMBER; MEMBER_COUNT];
    for (slot, member) in ordinals.iter_mut().zip(Member::ALL) {
        *slot = position_of(declared.clone(), member.name());
    }
    ordinals
}

/// The positions of `formals`, a block's formal names in order, among `declared`, its own
/// declaration names in order, as [`member_ordinals`] finds a member (checklist P1-5). A formal past
/// [`MAX_BUILTIN_FORMALS`] has no position, so it is found by its name.
pub(crate) fn formal_positions<'n, 'f>(
    declared: impl Iterator<Item = Option<&'n str>> + Clone,
    formals: impl Iterator<Item = &'f str>,
) -> FormalPositions {
    let mut positions = [NO_MEMBER; MAX_BUILTIN_FORMALS];
    for (slot, formal) in positions.iter_mut().zip(formals) {
        *slot = position_of(declared.clone(), formal);
    }
    positions
}

/// The position of the first of `declared` that is `name`, or [`NO_MEMBER`].
fn position_of<'n>(mut declared: impl Iterator<Item = Option<&'n str>>, name: &str) -> u16 {
    declared
        .position(|declared| declared == Some(name))
        .and_then(|position| u16::try_from(position).ok())
        .unwrap_or(NO_MEMBER)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each member sits at its own index of the table, so a position read for one member is never
    /// another member's.
    #[test]
    fn every_member_is_at_its_own_index() {
        for (index, member) in Member::ALL.into_iter().enumerate() {
            assert_eq!(member as usize, index, "{member:?}");
        }
    }

    /// TON declares IN and PT, then Q and ET (IEC 61131-3 Table 43), then its state slot: each member
    /// is at its declaration's position, the first of two equal names wins, and a member the block
    /// does not declare has none. Catches a position off by one, a later duplicate taken, and a
    /// member of another block found in this one.
    #[test]
    fn a_member_is_at_its_first_declaration_and_an_undeclared_one_at_none() {
        let declared = ["IN", "PT", "Q", "ET", STATE_LAST_TIME, "Q"];
        let ordinals = member_ordinals(declared.iter().map(|name| Some(*name)));
        assert_eq!(ordinals[Member::In as usize], 0);
        assert_eq!(ordinals[Member::Pt as usize], 1);
        assert_eq!(ordinals[Member::Q as usize], 2);
        assert_eq!(ordinals[Member::Et as usize], 3);
        assert_eq!(ordinals[Member::LastTime as usize], 4);
        assert_eq!(ordinals[Member::Clk as usize], NO_MEMBER);
        assert_eq!(ordinals[Member::Cv as usize], NO_MEMBER);
    }

    /// CTUD's eight formals take their declarations' positions in formal order, whatever order the
    /// declarations have; a formal no declaration names, and every index past the formals, has none.
    /// Catches positions taken in declaration order instead of formal order, and a formal past the
    /// table written over another.
    #[test]
    fn formals_take_their_declarations_positions_in_formal_order() {
        let declared = [
            "CV",
            "QD",
            "QU",
            "PV",
            "LD",
            "R",
            "CD",
            "CU",
            "__ST_PREV_CU",
        ];
        let formals = ["CU", "CD", "R", "LD", "PV", "QU", "QD", "CV"];
        let positions =
            formal_positions(declared.iter().map(|name| Some(*name)), formals.into_iter());
        assert_eq!(positions, [7, 6, 5, 4, 3, 2, 1, 0]);
        let positions = formal_positions(
            declared.iter().map(|name| Some(*name)),
            ["CU", "MISSING"].into_iter(),
        );
        assert_eq!(positions[0], 7);
        assert_eq!(positions[1], NO_MEMBER);
        assert!(positions[2..].iter().all(|&position| position == NO_MEMBER));
        let nine = [
            "CU",
            "CD",
            "R",
            "LD",
            "PV",
            "QU",
            "QD",
            "CV",
            "__ST_PREV_CU",
        ];
        let positions = formal_positions(declared.iter().map(|name| Some(*name)), nine.into_iter());
        assert_eq!(positions, [7, 6, 5, 4, 3, 2, 1, 0]);
    }

    /// A declaration whose name cannot be read takes its position but names no member.
    #[test]
    fn an_unreadable_name_keeps_its_position() {
        let declared = [None, Some("CLK"), Some("Q")];
        let ordinals = member_ordinals(declared.into_iter());
        assert_eq!(ordinals[Member::Clk as usize], 1);
        assert_eq!(ordinals[Member::Q as usize], 2);
    }

    fn declared(offset: u32, bytes: u32, type_id: u32, presence: bool) -> Option<DeclaredPlace> {
        Some(DeclaredPlace {
            offset,
            bytes,
            type_id,
            presence,
        })
    }

    fn place(offset: u16, bytes: u8, type_id: u32, presence: bool) -> Option<MemberPlace> {
        Some(MemberPlace {
            type_id,
            offset,
            bytes,
            presence,
        })
    }

    /// Each member takes the place of its first declaration, a state slot keeps its presence byte,
    /// and a member the block does not declare, or whose declaration has no place, has none; a
    /// declaration that is no member may lie anywhere, but a member's place past a `MemberPlace`
    /// leaves the block without places. Catches a place taken from a later declaration of the name,
    /// a lost presence byte, a table refused for a declaration no member uses, and an offset or a
    /// size cut to fit.
    #[test]
    fn members_take_the_places_of_their_first_declarations() {
        let block = [
            (Some("IN"), declared(0, 1, 1, false)),
            (Some("PT"), declared(8, 8, 18, false)),
            (Some("Q"), declared(16, 1, 1, false)),
            (Some("ET"), None),
            (Some("OTHER"), declared(70_000, 300, 3, false)),
            (Some(STATE_LAST_TIME), declared(24, 8, 19, true)),
            (Some("Q"), declared(40, 1, 1, false)),
        ];
        let places = member_places(block.iter().copied()).expect("every member's place fits");
        assert_eq!(places[Member::In as usize], place(0, 1, 1, false));
        assert_eq!(places[Member::Pt as usize], place(8, 8, 18, false));
        assert_eq!(places[Member::Q as usize], place(16, 1, 1, false));
        assert_eq!(places[Member::Et as usize], None);
        assert_eq!(places[Member::LastTime as usize], place(24, 8, 19, true));
        assert_eq!(places[Member::Clk as usize], None);
        let far = [(Some("IN"), declared(65_536, 1, 1, false))];
        assert_eq!(member_places(far.iter().copied()), None);
        let wide = [(Some("CLK"), declared(0, 256, 1, false))];
        assert_eq!(member_places(wide.iter().copied()), None);
    }

    /// A member's bytes lie at its offset from the instance's first byte, in the instance's region,
    /// and a state slot's presence byte right after its value; a member the block does not declare,
    /// or one past the store's offsets, has none. Catches an offset taken from the region's start,
    /// a presence byte at the value's start, and an offset that wraps.
    #[test]
    fn a_member_lies_at_its_offset_from_the_instance() {
        use crate::memory::layout::LayoutRegion;
        let mut members = [None; MEMBER_COUNT];
        members[Member::Q as usize] = place(16, 1, 1, false);
        members[Member::LastTime as usize] = place(24, 8, 19, true);
        let range = |start, bytes| ByteRange {
            region: LayoutRegion::Persistent,
            start,
            bytes,
        };
        let places = BlockPlaces {
            bytes: range(96, 48),
            members: &members,
        };
        assert_eq!(places.member(Member::Q), Some((range(112, 1), 1, None)));
        assert_eq!(
            places.member(Member::LastTime),
            Some((range(120, 8), 19, Some(128)))
        );
        assert_eq!(places.member(Member::In), None);
        let far = BlockPlaces {
            bytes: range(u32::MAX - 8, 48),
            members: &members,
        };
        assert_eq!(far.member(Member::Q), None);
    }
}
