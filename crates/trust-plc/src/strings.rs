//! The strings a Rust POU's variables have (specification 48 RU-25): `STRING[n]` and `WSTRING[n]`,
//! written `Str<n>` and `WStr<n>` (`STRING` is `STRING[80]`, specification 10 §2.3.1). Each is a
//! value the POU owns, its text and its length, which the generated code reads from the instance's
//! string as it lies in its area and writes back (specification 44 §6.2). A write into one keeps
//! the longest prefix of whole characters its capacity holds, as every write of Structured Text
//! does (specification 10 §2.3.1). None of their functions can panic (specification 48 RU-14).

use core::fmt;

use crate::types::Placed;

/// A `STRING[N]`: up to `N` bytes of UTF-8 text.
#[derive(Clone, Copy)]
pub struct Str<const N: usize = 80> {
    len: u16,
    bytes: [u8; N],
}

impl<const N: usize> Str<N> {
    /// The empty string, a `STRING`'s default (IEC §6.4.2, Table 10).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: 0,
            bytes: [0; N],
        }
    }

    /// The string of `bytes`, as many as `N` holds: the attributes give it a declared initial
    /// value, whose text they checked to fit (specification 48 RU-26).
    #[doc(hidden)]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn __from_bytes(bytes: &[u8]) -> Self {
        let mut value = Self::new();
        let mut index = 0;
        while index < bytes.len() && index < N && index < u16::MAX as usize {
            value.bytes[index] = bytes[index];
            index = index.saturating_add(1);
        }
        // The loop stops at 65,535.
        value.len = index as u16;
        value
    }

    /// The string of `text`, cut to the longest prefix of whole characters that `N` bytes hold.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut value = Self::new();
        value.set(text);
        value
    }

    /// Sets the text to `text`, cut to the longest prefix of whole characters that `N` bytes hold.
    pub fn set(&mut self, text: &str) {
        let mut end = text.len().min(N).min(usize::from(u16::MAX));
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        for (slot, byte) in self.bytes.iter_mut().zip(text.bytes()).take(end) {
            *slot = byte;
        }
        self.len = u16::try_from(end).unwrap_or(0);
    }

    /// Its text's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len()).unwrap_or(&[])
    }

    /// Its text; `None` for bytes that are not UTF-8, which a byte written by Structured Text can
    /// leave.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        core::str::from_utf8(self.as_bytes()).ok()
    }

    /// Its length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len).min(N)
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Its capacity in bytes, `N`.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }
}

impl<const N: usize> Default for Str<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> PartialEq for Str<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Eq for Str<N> {}

impl<const N: usize> fmt::Debug for Str<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(text) => write!(formatter, "Str<{N}>({text:?})"),
            None => write!(formatter, "Str<{N}>({:?})", self.as_bytes()),
        }
    }
}

impl<const N: usize> Placed for Str<N> {
    type Area = trust_plc_abi::Str<N>;

    fn load(area: &Self::Area) -> Self {
        let len = area.view().len();
        let mut value = Self::new();
        for (slot, cell) in value.bytes.iter_mut().zip(area.data.iter()).take(len) {
            *slot = cell.get();
        }
        value.len = u16::try_from(len).unwrap_or(0);
        value
    }

    fn store(&self, area: &Self::Area) {
        for (cell, byte) in area.data.iter().zip(self.as_bytes()) {
            cell.set(*byte);
        }
        // The length, and a 0 after the text when the capacity leaves room (DEV-039).
        area.view().set_len(self.len());
    }
}

/// A `WSTRING[N]`: up to `N` UTF-16 code units of text.
#[derive(Clone, Copy)]
pub struct WStr<const N: usize = 80> {
    len: u16,
    units: [u16; N],
}

impl<const N: usize> WStr<N> {
    /// The empty string, a `WSTRING`'s default.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: 0,
            units: [0; N],
        }
    }

    /// The string of `units`, as many as `N` holds: the attributes give it a declared initial
    /// value, whose text they checked to fit (specification 48 RU-26).
    #[doc(hidden)]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn __from_units(units: &[u16]) -> Self {
        let mut value = Self::new();
        let mut index = 0;
        while index < units.len() && index < N && index < u16::MAX as usize {
            value.units[index] = units[index];
            index = index.saturating_add(1);
        }
        // The loop stops at 65,535.
        value.len = index as u16;
        value
    }

    /// The string of `text`, cut to the longest prefix of whole characters that `N` code units
    /// hold.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut value = Self::new();
        value.set(text);
        value
    }

    /// Sets the text to `text`, cut to the longest prefix of whole characters that `N` code units
    /// hold.
    pub fn set(&mut self, text: &str) {
        let room = N.min(usize::from(u16::MAX));
        let mut len: usize = 0;
        let mut buffer = [0_u16; 2];
        for character in text.chars() {
            let encoded = character.encode_utf16(&mut buffer);
            if len.saturating_add(encoded.len()) > room {
                break;
            }
            for (slot, unit) in self.units.iter_mut().skip(len).zip(encoded.iter()) {
                *slot = *unit;
            }
            len = len.saturating_add(encoded.len());
        }
        self.len = u16::try_from(len).unwrap_or(0);
    }

    /// Its code units.
    #[must_use]
    pub fn as_units(&self) -> &[u16] {
        self.units.get(..self.len()).unwrap_or(&[])
    }

    /// Its characters; a code unit that is not part of a character reads as `U+FFFD`.
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        char::decode_utf16(self.as_units().iter().copied())
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    /// Its length in code units.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len).min(N)
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Its capacity in code units, `N`.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }
}

impl<const N: usize> Default for WStr<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> PartialEq for WStr<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_units() == other.as_units()
    }
}

impl<const N: usize> Eq for WStr<N> {}

impl<const N: usize> fmt::Debug for WStr<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "WStr<{N}>(\"")?;
        for character in self.chars() {
            write!(formatter, "{}", character.escape_debug())?;
        }
        write!(formatter, "\")")
    }
}

impl<const N: usize> Placed for WStr<N> {
    type Area = trust_plc_abi::WStr<N>;

    fn load(area: &Self::Area) -> Self {
        let len = area.view().len();
        let mut value = Self::new();
        for (slot, cell) in value.units.iter_mut().zip(area.data.iter()).take(len) {
            *slot = cell.get();
        }
        value.len = u16::try_from(len).unwrap_or(0);
        value
    }

    fn store(&self, area: &Self::Area) {
        for (cell, unit) in area.data.iter().zip(self.as_units()) {
            cell.set(*unit);
        }
        // The length, and a 0 after the text when the capacity leaves room (DEV-039).
        area.view().set_len(self.len());
    }
}
