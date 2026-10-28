//! The layout of a reference (specification 44 §7.3).

/// A reference as it lies in an area: 16 bytes on every target (specification 44 §7.3). A `REF_TO`
/// variable, an interface reference and a `POINTER TO` variable hold one.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefValue {
    /// The id of the target's area; 0 with kind [`RefValue::NULL`].
    pub area: u8,
    /// What the reference names: [`RefValue::NULL`], [`RefValue::VARIABLE`],
    /// [`RefValue::INSTANCE`], [`RefValue::CHARACTER`] or [`RefValue::POINTER`].
    pub kind: u8,
    /// The 1-based index of a character reference's character, 0 otherwise.
    pub char_index: u16,
    /// The target's first byte in its area.
    pub offset: u32,
    /// The bytes the target spans.
    pub extent: u32,
    /// The installation generation the reference was taken in (specification 34 UPDATE-04).
    pub generation: u32,
}

impl RefValue {
    /// `NULL`: no area, no target.
    pub const NULL_REF: Self = Self {
        area: 0,
        kind: Self::NULL,
        char_index: 0,
        offset: 0,
        extent: 0,
        generation: 0,
    };

    /// The kind of `NULL`.
    pub const NULL: u8 = 0;
    /// The kind of a reference to a variable.
    pub const VARIABLE: u8 = 1;
    /// The kind of a reference to a function block or class instance.
    pub const INSTANCE: u8 = 2;
    /// The kind of a reference to one character of a string.
    pub const CHARACTER: u8 = 3;
    /// The kind of a `POINTER TO` value.
    pub const POINTER: u8 = 4;
}
