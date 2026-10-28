//! The layout of a string (specification 44 §6.2, specification 34 MEM-12).

use core::cell::Cell;

/// A `STRING[N]` as it lies in an area: its length in bytes and `N` bytes of UTF-8, `2 + N` bytes
/// rounded up to an even number, so that the length of each element of an array is aligned
/// (specification 44 §7.2). `N` is at most 65,535 (specification 44 §7.6).
#[repr(C)]
pub struct Str<const N: usize> {
    /// The length in bytes; a length above `N` reads as `N`.
    pub len: Cell<u16>,
    /// The bytes; those after the length are not part of the value.
    pub data: [Cell<u8>; N],
}

impl<const N: usize> Str<N> {
    /// The empty string, as a temporary of a function holds it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: Cell::new(0),
            data: [const { Cell::new(0) }; N],
        }
    }

    /// The string as the program library reads and writes it, whatever its capacity.
    #[must_use]
    pub fn view(&self) -> StrView<'_> {
        StrView {
            len: &self.len,
            data: &self.data,
        }
    }
}

impl<const N: usize> Default for Str<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// A `WSTRING[N]` as it lies in an area: its length in UTF-16 code units and `N` code units,
/// `2 + 2N` bytes.
#[repr(C)]
pub struct WStr<const N: usize> {
    /// The length in code units; a length above `N` reads as `N`.
    pub len: Cell<u16>,
    /// The code units; those after the length are not part of the value.
    pub data: [Cell<u16>; N],
}

impl<const N: usize> WStr<N> {
    /// The empty string, as a temporary of a function holds it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: Cell::new(0),
            data: [const { Cell::new(0) }; N],
        }
    }

    /// The string as the program library reads and writes it, whatever its capacity.
    #[must_use]
    pub fn view(&self) -> WStrView<'_> {
        WStrView {
            len: &self.len,
            data: &self.data,
        }
    }
}

impl<const N: usize> Default for WStr<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// A `STRING` of any capacity: its length and its bytes.
#[derive(Clone, Copy)]
pub struct StrView<'a> {
    /// The length in bytes.
    pub len: &'a Cell<u16>,
    /// The bytes, as many as the capacity.
    pub data: &'a [Cell<u8>],
}

impl StrView<'_> {
    /// The capacity in bytes.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    /// The length in bytes, at most the capacity.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len.get()).min(self.data.len())
    }

    /// Whether the string is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The byte at `index`, 0-based, or 0 past the length.
    #[must_use]
    pub fn byte(&self, index: usize) -> u8 {
        if index < self.len() {
            self.data.get(index).map_or(0, Cell::get)
        } else {
            0
        }
    }
}

/// A `WSTRING` of any capacity: its length and its code units.
#[derive(Clone, Copy)]
pub struct WStrView<'a> {
    /// The length in code units.
    pub len: &'a Cell<u16>,
    /// The code units, as many as the capacity.
    pub data: &'a [Cell<u16>],
}

impl WStrView<'_> {
    /// The capacity in code units.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    /// The length in code units, at most the capacity.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len.get()).min(self.data.len())
    }

    /// Whether the string is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
