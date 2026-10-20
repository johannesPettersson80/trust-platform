//! Flat references (specification 34 MEM-12; C1-11d).
//!
//! On the flat store a reference to a whole variable, member or element in the persistent block is
//! the block and byte offset of its target, its type and its generation: the 16 bytes a reference
//! leaf holds. A reference the program takes is made flat as soon as its path names such a target,
//! so a reference variable, a copy or a comparison of it holds no path. A reference to a local, to
//! a character of a text, to a declaration that cannot be written and one taken while an
//! initializer runs keeps its root and path, which the lifetime, mutability and staging checks
//! read. A flat reference keeps the element count of the path it was taken through, which its
//! copies are charged by, as the value store charged the copy of the path (TIME-08; C1-19e).

use super::*;
use crate::memory::layout::LayoutRegion;
use crate::memory::MemoryLocation;
use crate::value::{path_elements, RefPath};

impl EngineState<'_> {
    /// `reference` made flat when it names a whole writable target in the persistent block; the
    /// buffers of its path go back to the engine. Any other reference comes back as it is.
    pub(super) fn flatten_reference(&self, reference: ValueRef) -> ValueRef {
        let Some(flat) = self.flat_form(&reference) else {
            return reference;
        };
        self.data.resources.references.recycle(reference.path);
        flat
    }

    fn flat_form(&self, reference: &ValueRef) -> Option<ValueRef> {
        if !self.data.construction.initializers.is_empty()
            || self
                .check_writable_declaration(reference.as_view())
                .is_err()
        {
            return None;
        }
        let reader = self.data.reader(self.prepared);
        let (range, type_id, presence) = reader.cells_target(
            reference.location,
            reference.offset as usize,
            &reference.path,
        )?;
        if range.region != LayoutRegion::Persistent || presence.is_some() {
            return None;
        }
        let elements = reference
            .location
            .flat_elements()
            .saturating_add(path_elements(&reference.path));
        Some(ValueRef {
            location: MemoryLocation::Flat {
                region: range.region,
                elements: u16::try_from(elements).unwrap_or(u16::MAX),
                type_id,
            },
            offset: range.start,
            generation: reference.generation,
            path: RefPath::new(),
        })
    }
}

impl EngineState<'_> {
    /// The root-and-path form of `reference` when it is flat: the variable that holds its target
    /// and the path to it, as the value store names it (C1-11d). Read at the boundary, not in a
    /// scan; any other reference comes back as it is.
    pub(crate) fn path_form(&self, reference: &ValueRef) -> ValueRef {
        let reader = self.data.reader(self.prepared);
        reader
            .path_reference(reference)
            .unwrap_or_else(|| reference.clone())
    }
}
