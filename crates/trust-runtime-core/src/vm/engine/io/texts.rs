//! Byte-string windows of the input image written into text leaves where they lie (specification
//! 34 ARCH-05; C1-16).
//!
//! A `STRING` bound to a byte window of the input image is sampled as the value path samples it: the
//! image read's checks and charge, the write's checks and the normalization's charges to the
//! declared `STRING`, in the same order. When its destination is a text leaf of the flat store, the
//! text is then written into the leaf from the image, cut to the leaf's capacity, so sampling builds
//! no value.

use super::staging::StagedValue;
use super::*;
use crate::io_image::PreparedIoBinding;
use crate::memory::cell_access::PlaceTarget;
use crate::memory::cells::text_geometry;
use crate::memory::layout::ByteRange;
use crate::stdlib::conversions::ConversionType;
use crate::vm::engine::store_access::Routed;
use crate::vm::prepared::byte_types::text_primitive;
use core::ops::Range;

/// A text leaf of the flat store that an input writes: its bytes, its geometry and the presence byte
/// of its declaration.
struct TextLeaf {
    range: ByteRange,
    geometry: (u32, u32, bool),
    presence: Option<u32>,
}

impl EngineState<'_> {
    /// Stage the byte-string input of `binding` as a window of the input image, when its
    /// destination `reference` is a `STRING` leaf of the flat store: the checks and charges of
    /// reading the window as a value, checking the write of that value and normalizing it to the
    /// declared `STRING`, as the value path makes them. `None`, having charged nothing, when the
    /// value path samples it.
    pub(super) fn stage_text_input(
        &self,
        binding: &PreparedIoBinding,
        reference: ValueRefView<'_>,
    ) -> Result<Option<StagedValue>, RuntimeError> {
        let address = &binding.address;
        let crate::io_address::IoSize::Bytes(len) = address.size else {
            return Ok(None);
        };
        let Some(ty) = binding.type_id else {
            return Ok(None);
        };
        // A `STRING` binding's codec converts a text to itself.
        if address.path.len() > 1
            || !matches!(binding.codec.wire_type, None | Some(ConversionType::String))
            || binding.codec.enum_type.is_some()
        {
            return Ok(None);
        }
        let Some((24, max_length)) = text_primitive(self.prepared, ty) else {
            return Ok(None);
        };
        if self.text_leaf(reference).is_none() {
            return Ok(None);
        }
        // The image read's checks and charge.
        validate_process_image_address(address)?;
        self.charge_allocation_bytes(len as usize)?;
        let image = image_of(&self.data.images, address.area);
        self.check_image_window(address, image.len())?;
        let start = address.byte as usize;
        let end = image.len().min(start.saturating_add(len as usize));
        let window = image.get(start..end).unwrap_or_default();
        let nul = window
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(window.len());
        let text = core::str::from_utf8(&window[..nul]).map_err(|_| RuntimeError::TypeMismatch)?;
        let read = text.len();
        // The write's checks: a text holds no reference, so they are its target's.
        self.check_write_target(reference)?;
        // The normalization to the declared `STRING`.
        self.charge_work_units(read.checked_mul(2).ok_or(RuntimeError::Overflow)?)?;
        let capacity = usize::from(crate::value::string_capacity(max_length));
        let kept = match crate::value::string_capacity_cut(text, false, capacity) {
            Some(cut) => {
                let kept =
                    crate::vm::engine::allocation::text_copy_allocation(&text[..cut], false)?;
                self.charge_allocation_bytes(kept)?;
                cut
            }
            None => read,
        };
        Ok(Some(StagedValue::Text {
            area: address.area,
            start: address.byte,
            len: u32::try_from(kept).map_err(|_| RuntimeError::Overflow)?,
        }))
    }

    /// Write the staged text `bytes` of the image of `area` into the text leaf that the slot
    /// `offset` of `location` and `path` name, as the value path writes the text's value there: the
    /// path write's charge, the destination's checks, a sample of the deadline, then the write.
    pub(super) fn write_text_input(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        area: IoArea,
        bytes: Range<usize>,
    ) -> Result<(), RuntimeError> {
        let reference = ValueRefView {
            location,
            offset,
            generation: StoreAccess::generation(self),
            path,
        };
        // The charge depends on the kind of the written value, not on its text.
        let kind = Value::String(smol_str::SmolStr::new_inline(""));
        self.charge_storage_path_write(&*self, location, offset, path, &kind)?;
        let leaf = self
            .text_leaf(reference)
            .ok_or(RuntimeError::NullReference)?;
        self.check_entry_deadline()?;
        let image = image_of(&self.data.images, area);
        let text = image
            .get(bytes)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let EngineStore::Cells(arm) = &mut self.data.store else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if arm.write_text(leaf.range, leaf.geometry, leaf.presence, text) {
            Ok(())
        } else {
            Err(RuntimeError::NullReference)
        }
    }

    /// The `STRING` leaf of the flat store that `reference` names, or `None` for any other
    /// destination.
    fn text_leaf(&self, reference: ValueRefView<'_>) -> Option<TextLeaf> {
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return None;
        };
        let PlaceTarget::Cells {
            range,
            type_id,
            presence,
            ..
        } = reader.place_target(reference.location, reference.offset, reference.path)?
        else {
            return None;
        };
        let geometry = text_geometry(self.prepared, type_id).filter(|(_, _, wide)| !wide)?;
        Some(TextLeaf {
            range,
            geometry,
            presence,
        })
    }
}

/// The flat image of `area`.
fn image_of(images: &ProcessImages, area: IoArea) -> &[u8] {
    match area {
        IoArea::Input => &images.inputs,
        IoArea::Output => &images.outputs,
        IoArea::Memory => &images.memory,
    }
}
