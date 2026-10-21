//! The variables a standard function block's native code reads and writes, and their places among
//! its POU's own declarations (checklist P1-4; specification 34 PERF-05).
//!
//! Preparation finds each member by its name among the block's own declarations once, so a scan
//! reaches a persistent instance's member by its position and compares no names. A position is the
//! one the name lookup finds, so a block reads and writes the same variables either way.

use super::state::{
    STATE_ACTIVE, STATE_LAST_TIME, STATE_PREV_CD, STATE_PREV_CU, STATE_PREV_IN, STATE_TIMING,
    STATE_TRIG_M,
};

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
}
