//! The text buffers of the bounded value store and the sink that writes into them
//! (specification 34 §6.2; design "Regions" and "Boundaries").
//!
//! A region owns one [`TextStore`](crate::memory::text::TextStore): one fixed buffer per text leaf
//! it holds, each reserved once at its leaf's byte capacity and never grown. A
//! [`Cell::Text`](crate::memory::cells::Cell::Text) names its buffer by
//! [`TextSlot`](crate::memory::cells::TextSlot), the cell offset of the leaf within the region. A
//! [`TextSink`](crate::memory::text::TextSink) writes a text into a buffer and stops exactly where
//! assignment normalization stops: on a Unicode scalar boundary for a `STRING` (UTF-8 bytes) and
//! on a UTF-16 code unit pair boundary for a `WSTRING`, so the stored text is the normalized
//! `Value` (`docs/IEC_DECISIONS.md` 2026-10-17).

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::memory::cells::TextSlot;
use crate::memory::layout::{LayoutRegion, TextLeaf};
use crate::value::string_capacity_cut;
use crate::vm::VmTrap;

/// One text leaf's fixed buffer (design "Regions").
#[derive(Debug, Default)]
struct TextBuffer {
    /// The cell offset of the leaf within the region: its slot.
    cell: u32,
    /// The bytes reserved for the text (`TextLeaf::capacity_bytes`), never grown.
    capacity: usize,
    /// The text in use: UTF-8 bytes for a `STRING`, little-endian UTF-16 code units for a
    /// `WSTRING`. Never longer than `capacity`.
    bytes: Vec<u8>,
}

/// The text buffers of one region (specification 34 §6.2; design "Regions").
///
/// [`TextStore::reserve_in`] reserves one buffer per text leaf, so the store never grows; every
/// accessor answers `None` for a slot the store does not hold, and reading a text allocates only
/// the materialized value.
#[derive(Debug)]
pub struct TextStore {
    region: LayoutRegion,
    buffers: Vec<TextBuffer>,
}

impl Default for TextStore {
    /// The empty persistent region: no text buffers.
    fn default() -> Self {
        Self {
            region: LayoutRegion::Persistent,
            buffers: Vec::new(),
        }
    }
}

impl TextStore {
    /// Reserve the persistent region's buffers, one per leaf of `leaves` (specification 34 §6.2;
    /// design "Regions").
    ///
    /// [`TextStore::reserve_in`] reserves a region of another [`LayoutRegion`].
    ///
    /// # Errors
    ///
    /// Refused as the other construction reservations are when a buffer cannot be reserved, and
    /// with [`RuntimeError::TypeMismatch`] when `leaves` is not in strictly increasing cell order:
    /// one buffer belongs to one leaf cell.
    pub fn reserve(leaves: &[TextLeaf]) -> Result<Self, RuntimeError> {
        Self::reserve_in(LayoutRegion::Persistent, leaves)
    }

    /// Reserve a region's buffers, one per leaf of `leaves` (specification 34 §6.2; design
    /// "Regions").
    ///
    /// Each buffer reserves exactly its leaf's `capacity_bytes` once and never grows, and its slot
    /// is the leaf's cell offset. A region reserves one leaf per occurrence: an array of text
    /// elements contributes its element layout's leaves once per element, at
    /// `element × stride + leaf.offset`.
    ///
    /// # Errors
    ///
    /// Refused as [`TextStore::reserve`] is.
    pub fn reserve_in(region: LayoutRegion, leaves: &[TextLeaf]) -> Result<Self, RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        let mut store = Self {
            region,
            ..Self::default()
        };
        store
            .buffers
            .try_reserve_exact(leaves.len())
            .map_err(refused)?;
        let mut previous: Option<u32> = None;
        for leaf in leaves {
            if previous.is_some_and(|previous| leaf.offset <= previous) {
                return Err(RuntimeError::TypeMismatch);
            }
            previous = Some(leaf.offset);
            let capacity =
                usize::try_from(leaf.capacity_bytes).map_err(|_| RuntimeError::Overflow)?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(capacity).map_err(refused)?;
            store.buffers.push(TextBuffer {
                cell: leaf.offset,
                capacity,
                bytes,
            });
        }
        Ok(store)
    }

    /// The region this store's buffers are reserved in (design "Regions").
    #[must_use]
    pub fn region(&self) -> LayoutRegion {
        self.region
    }

    /// The buffers and the bytes each of them reserves.
    ///
    /// [`TextStore::reserve_in`] fixes the reservation and no later write reallocates a buffer, so
    /// the answer never grows from the bytes reserved once (specification 34 §6.2; design
    /// "Regions").
    #[must_use]
    pub fn reserved(&self) -> (usize, usize) {
        (
            self.buffers.len(),
            self.buffers
                .iter()
                .map(|buffer| buffer.bytes.capacity())
                .sum(),
        )
    }

    /// The bytes reserved for the text at `slot`, or `None` when the store holds no such slot.
    #[must_use]
    pub fn capacity(&self, slot: TextSlot) -> Option<usize> {
        let index = self.index(slot)?;
        self.buffers.get(index).map(|buffer| buffer.capacity)
    }

    /// The bytes in use at `slot`, or `None` when the store holds no such slot.
    #[must_use]
    pub fn length(&self, slot: TextSlot) -> Option<usize> {
        let index = self.index(slot)?;
        self.buffers.get(index).map(|buffer| buffer.bytes.len())
    }

    /// The text bytes in use at `slot`, or `None` when the store holds no such slot.
    #[must_use]
    pub fn bytes(&self, slot: TextSlot) -> Option<&[u8]> {
        let index = self.index(slot)?;
        self.buffers
            .get(index)
            .map(|buffer| buffer.bytes.as_slice())
    }

    /// The text at `slot` materialized as a `STRING` (`wide` false) or a `WSTRING` (`wide` true),
    /// or `None` when the store holds no such slot or its bytes are not a text of that width.
    #[must_use]
    pub fn text(&self, slot: TextSlot, wide: bool) -> Option<String> {
        let bytes = self.bytes(slot)?;
        if wide {
            if !bytes.len().is_multiple_of(2) {
                return None;
            }
            core::char::decode_utf16(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit)),
            )
            .collect::<Result<String, _>>()
            .ok()
        } else {
            String::from_utf8(bytes.to_vec()).ok()
        }
    }

    /// Write `text` into `slot`, replacing the stored text and cutting it where assignment
    /// normalization cuts it (C1-7).
    ///
    /// Answers the bytes written, or `None` when the store holds no such slot.
    pub fn write(&mut self, slot: TextSlot, wide: bool, text: &str) -> Option<usize> {
        let mut sink = self.sink(slot, wide)?;
        sink.clear();
        Some(sink.write_str(text))
    }

    /// A writer that appends to the text at `slot`, or `None` when the store holds no such slot
    /// (design "Standard library": a bounded output sink).
    pub fn sink(&mut self, slot: TextSlot, wide: bool) -> Option<TextSink<'_>> {
        let index = self.index(slot)?;
        let buffer = self.buffers.get_mut(index)?;
        Some(TextSink { buffer, wide })
    }

    /// The buffer index of `slot`, found by its cell offset.
    fn index(&self, slot: TextSlot) -> Option<usize> {
        self.buffers
            .binary_search_by_key(&slot.0, |buffer| buffer.cell)
            .ok()
    }
}

/// A writer into one text buffer (specification 34 §6.2; design "Standard library": a bounded
/// output sink).
///
/// `wide` selects the unit of the buffer's capacity: UTF-8 bytes for a `STRING`, UTF-16 code units
/// for a `WSTRING`. A write keeps only the prefix of whole scalars that fits and never splits a
/// surrogate pair, so it stops where assignment normalization stops and allocates nothing.
#[derive(Debug)]
pub struct TextSink<'a> {
    buffer: &'a mut TextBuffer,
    wide: bool,
}

impl TextSink<'_> {
    /// Append the prefix of `text` that fits, cutting it where assignment normalization cuts it,
    /// and answer the bytes appended.
    #[must_use]
    pub fn write_str(&mut self, text: &str) -> usize {
        let free = self.buffer.capacity.saturating_sub(self.buffer.bytes.len());
        let capacity = if self.wide { free / 2 } else { free };
        let end = string_capacity_cut(text, self.wide, capacity).unwrap_or(text.len());
        let prefix = &text[..end];
        let before = self.buffer.bytes.len();
        if self.wide {
            self.buffer
                .bytes
                .extend(prefix.encode_utf16().flat_map(u16::to_le_bytes));
        } else {
            self.buffer.bytes.extend_from_slice(prefix.as_bytes());
        }
        self.buffer.bytes.len() - before
    }

    /// Remove the text in use, keeping the reserved capacity.
    pub fn clear(&mut self) {
        self.buffer.bytes.clear();
    }
}

#[cfg(test)]
#[path = "text/tests.rs"]
mod tests;
