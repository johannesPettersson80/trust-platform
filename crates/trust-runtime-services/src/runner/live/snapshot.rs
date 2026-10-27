//! The I/O snapshot that a scan publishes for the control requests (checklist P1-6; specification
//! 34 PERF-05): each binding's value, read from the process image at the end of the scan.
//!
//! A device whose bindings are all flat scalars publishes a copy of the flat images with the scan
//! and the forced addresses, and the bindings are decoded from the copy when a request reads the
//! snapshot, so a scan decodes no binding. A device that binds a text window or a hierarchical
//! value publishes the decoded snapshot every scan.

use alloc::string::ToString;
use alloc::vec::Vec;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::io_image::{FlatImages, PreparedIoBinding};
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::value::Value;

use crate::live_values::apply;
use crate::live_values::io_snapshot::{IoSnapshot, IoSnapshotValue};

/// Whether the snapshot of `bindings` is published as a copy of the flat images: every binding is
/// a flat scalar, whose read charges the scan nothing.
pub(super) fn copies_images(bindings: &[PreparedIoBinding]) -> bool {
    bindings.iter().all(|binding| {
        binding.address.path.len() <= 1 && !matches!(binding.address.size, IoSize::Bytes(_))
    })
}

/// What the last scan published as a copy: its number, the addresses that a force held and the flat
/// images.
#[derive(Debug, Default)]
pub(super) struct PublishedImages {
    scan: Option<u64>,
    forced: Vec<IoAddress>,
    images: FlatImages,
}

impl PublishedImages {
    /// Record scan `scan`, the addresses `forced` and a copy of `images`, the process images of a
    /// compiled program, keeping the storage of the copy.
    pub(super) fn record_images<'f>(
        &mut self,
        images: &FlatImages,
        scan: u64,
        forced: impl Iterator<Item = &'f IoAddress>,
    ) {
        self.scan = Some(scan);
        self.forced.clear();
        self.forced.extend(forced.cloned());
        self.images.inputs.clone_from(&images.inputs);
        self.images.outputs.clone_from(&images.outputs);
        self.images.memory.clone_from(&images.memory);
    }

    /// Whether a scan has published.
    pub(super) fn published(&self) -> bool {
        self.scan.is_some()
    }

    /// Write the scan, the forced addresses and the value of each of `bindings` into `snapshot`,
    /// which was laid out for them, from the copy.
    pub(super) fn decode_into(
        &self,
        bindings: &[PreparedIoBinding],
        snapshot: &mut IoSnapshot,
    ) -> Result<(), RuntimeError> {
        snapshot.scan = self.scan;
        snapshot.forced.clone_from(&self.forced);
        fill(bindings, snapshot, |address| self.images.read(address))
    }
}

/// Write the value of each of `bindings` into its entry of `snapshot`, which was laid out in binding
/// order, reading each binding's raw value with `read`: the decoded value, or the error that its
/// read or its decoding gave. An error that ends the scan, such as a passed deadline, is returned.
pub(super) fn fill(
    bindings: &[PreparedIoBinding],
    snapshot: &mut IoSnapshot,
    mut read: impl FnMut(&IoAddress) -> Result<Value, RuntimeError>,
) -> Result<(), RuntimeError> {
    // The entries were laid out in binding order, so the same order updates their values without
    // changing the snapshot's shape or allocating.
    let mut inputs = 0;
    let mut outputs = 0;
    let mut memory = 0;
    for binding in bindings {
        let value = match read(&binding.address) {
            Ok(raw) => match binding.codec.decode(raw) {
                Ok(value) => IoSnapshotValue::Value(value),
                Err(error) => IoSnapshotValue::Error(error.to_string()),
            },
            Err(error) if apply::is_cycle_operational_error(&error) => return Err(error),
            Err(error) => IoSnapshotValue::Error(error.to_string()),
        };
        let entry = match binding.address.area {
            IoArea::Input => {
                let entry = snapshot.inputs.get_mut(inputs);
                inputs += 1;
                entry
            }
            IoArea::Output => {
                let entry = snapshot.outputs.get_mut(outputs);
                outputs += 1;
                entry
            }
            IoArea::Memory => {
                let entry = snapshot.memory.get_mut(memory);
                memory += 1;
                entry
            }
        };
        entry.ok_or(RuntimeError::InvalidExecutionState)?.value = value;
    }
    Ok(())
}
