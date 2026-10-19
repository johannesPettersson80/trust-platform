//! Copies of places that a call leaves on the operand stack (specification 34 MEM-12, ARCH-05;
//! C1-19b).
//!
//! A call between the load of an array or a structure and its store may write the bytes the place
//! stands for, as `pairs[Bump(target := pairs[1].a)] := pairs[1]` writes `pairs[1].a`, and the
//! store must store the value its load read. So before the call the dispatcher copies the bytes of
//! each place the call leaves on the stack into the stack's temporary area. A store writes the copy
//! to a destination of its type, checked and charged as a copy of the place is (TIME-08), and an
//! instruction that pops it as a value reads it into one, as it reads the place. Such an assignment
//! builds no value.

use super::*;

impl EngineState<'_> {
    /// The bytes of `place`, which a call may copy for the store after it.
    pub(in crate::vm::engine) fn place_copy(&self, place: Place) -> Option<&[u8]> {
        match self.data.routed(self.prepared) {
            Routed::Cells(reader) => reader.view_cells().bytes(place.range),
            Routed::Values(_) => None,
        }
    }

    /// Charge what normalizing the value the copy `bytes` of `place` stands for to its own type
    /// charges, as [`Self::charge_place_normalization`] charges it for the place: its texts'
    /// capacity checks, each text read from the copy.
    pub(in crate::vm::engine) fn charge_copy_normalization(
        &self,
        bytes: &[u8],
        place: Place,
    ) -> Result<(), RuntimeError> {
        if !holds_text(self.prepared, place.type_id) {
            return Ok(());
        }
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        self.charge_texts(
            (reader, Some(bytes)),
            place.range,
            place.type_id,
            TextRule::Normalization {
                in_structure: false,
            },
            0,
        )
    }

    /// Write the copy `bytes` to `destination`, the bytes of a value of the copy's type in the flat
    /// block that are no node of a backing, as `copy_place` writes a place there: the deadline
    /// sampled first when the value store stages the same write as a physical patch (C1-15b).
    /// `Ok(false)` when the block refuses the destination.
    pub(in crate::vm::engine) fn write_copy(
        &mut self,
        bytes: &[u8],
        destination: PlaceDestination,
    ) -> Result<bool, RuntimeError> {
        if destination.node.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        if destination.physical {
            self.check_entry_deadline()?;
        }
        Ok(match &mut self.data.store {
            EngineStore::Cells(arm) => {
                arm.write_copy(destination.range, destination.presence, bytes)
            }
            EngineStore::Values => false,
        })
    }
}
