//! The layout of a string (specification 44 §6.2, specification 34 MEM-12).

use core::cell::Cell;

/// A `STRING[N]` as it lies in an area: its length in bytes and `N` bytes of UTF-8, `2 + N` bytes
/// rounded up to an even number, so that the length of each element of an array is aligned
/// (specification 44 §7.2). `N` is at most 65,535 (specification 44 §7.6).
#[repr(C)]
pub struct Str<const N: usize> {
    /// The length in bytes; a length above `N` reads as `N`.
    pub len: Cell<u16>,
    /// The bytes. A write of the text puts a 0 after it when the length is below `N`
    /// ([`StrView::set_len`]); the bytes after the length are not part of the value.
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
    /// The code units. A write of the text puts a 0 after it when the length is below `N`
    /// ([`WStrView::set_len`]); the units after the length are not part of the value.
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

    /// Sets the length to `len` bytes, at most the capacity, after a write of the text, and puts a
    /// 0 byte after the text when the capacity leaves room, keeping the bytes after that 0. CODESYS
    /// lays a string out as its text and a terminating 0, and its profile's `ADR(s)` names these
    /// bytes (specification 44 §6.2, DEV-039 in `docs/IEC_DEVIATIONS.md`).
    pub fn set_len(&self, len: usize) {
        let len = len.min(self.data.len());
        // The capacity is at most 65,535 (specification 44 §7.6).
        self.len.set(u16::try_from(len).unwrap_or(u16::MAX));
        if let Some(terminator) = self.data.get(len) {
            terminator.set(0);
        }
    }

    /// Sets the length that the bytes `first..end` give the text after a write through a pointer to
    /// the string's bytes: the text ends at its first 0 byte, or at the capacity, as CODESYS reads
    /// a string (`docs/IEC_DEVIATIONS.md` DEV-039). The bytes before `first` are as they were, and
    /// none of them before the length is 0, so a 0 written before the length ends the text there; a
    /// byte other than 0 written over the 0 after the text extends the text to the next 0, which may
    /// be one an earlier, longer text left, or to the capacity; a write after that 0 leaves the
    /// text as it was. Only the length changes: the bytes are what was written.
    pub fn bytes_written(&self, first: usize, end: usize) {
        if let Some(len) = written_length(self.data, self.len(), first, end) {
            // The capacity is at most 65,535 (specification 44 §7.6).
            self.len.set(u16::try_from(len).unwrap_or(u16::MAX));
        }
    }
}

/// The length that the elements `first..end`, written into `data`, give a text that was `len`
/// long, as [`StrView::bytes_written`] describes it: the text ends at its first 0, or at the
/// capacity. `None` when the length stays as it was.
fn written_length<T: Copy + Default + PartialEq>(
    data: &[Cell<T>],
    len: usize,
    first: usize,
    end: usize,
) -> Option<usize> {
    let end = end.min(data.len());
    if first > len {
        return None;
    }
    let zero = |from: usize, to: usize| {
        data.get(from..to)
            .and_then(|elements| {
                elements
                    .iter()
                    .position(|element| element.get() == T::default())
            })
            .map(|position| from + position)
    };
    match zero(first, end) {
        Some(written) => Some(written),
        // The 0 after the text, or the capacity, still ends it.
        None if end <= len => None,
        None => Some(zero(end, data.len()).unwrap_or(data.len())),
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

    /// Sets the length to `len` code units, at most the capacity, after a write of the text, and
    /// puts a 0 code unit after the text when the capacity leaves room, like
    /// [`StrView::set_len`].
    pub fn set_len(&self, len: usize) {
        let len = len.min(self.data.len());
        // The capacity is at most 65,535 (specification 44 §7.6).
        self.len.set(u16::try_from(len).unwrap_or(u16::MAX));
        if let Some(terminator) = self.data.get(len) {
            terminator.set(0);
        }
    }

    /// Sets the length that the code units `first..end` give the text after a write of them, as
    /// the CODESYS profile writes `ws[i]` (`docs/IEC_DEVIATIONS.md` DEV-046): the text ends at its
    /// first 0 unit, or at the capacity, as [`StrView::bytes_written`] sets a string's length from
    /// its bytes. Only the length changes: the units are what was written.
    pub fn units_written(&self, first: usize, end: usize) {
        if let Some(len) = written_length(self.data, self.len(), first, end) {
            // The capacity is at most 65,535 (specification 44 §7.6).
            self.len.set(u16::try_from(len).unwrap_or(u16::MAX));
        }
    }
}
