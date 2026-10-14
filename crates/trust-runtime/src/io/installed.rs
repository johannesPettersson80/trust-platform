//! Immutable hosted presentation of the shared engine's process images.
use super::*;
use trust_runtime_core::vm::{ExecutionServices, OwnedRuntimeState};

// Erase only the platform service type. This is a borrowed read-only projection,
// not another image, dispatcher or installed-state mutation interface.
trait InstalledImages {
    fn inputs(&self) -> &[u8];
    fn outputs(&self) -> &[u8];
    fn memory(&self) -> &[u8];
    fn hierarchical(&self, address: &IoAddress) -> Option<&Value>;
}
impl<S: ExecutionServices> InstalledImages for OwnedRuntimeState<S> {
    fn inputs(&self) -> &[u8] {
        self.inputs()
    }
    fn outputs(&self) -> &[u8] {
        self.outputs()
    }
    fn memory(&self) -> &[u8] {
        self.memory_image()
    }
    fn hierarchical(&self, address: &IoAddress) -> Option<&Value> {
        self.read_hierarchical_image_ref(address)
    }
}

/// Borrowed images and immutable presentation bindings for an installed runtime.
/// Writes use the controller's checked `write_io` operation. This view cannot
/// resize images, change bindings or mutate live storage.
///
/// ```compile_fail,E0599
/// use trust_runtime::io::InstalledIoView;
/// fn bypass(view: &mut InstalledIoView<'_>) {
///     let _ = view.outputs_mut();
/// }
/// ```
pub struct InstalledIoView<'a> {
    schema: &'a IoInterface,
    images: &'a dyn InstalledImages,
}
impl<'a> InstalledIoView<'a> {
    pub(crate) fn new<S: ExecutionServices>(
        schema: &'a IoInterface,
        state: &'a OwnedRuntimeState<S>,
    ) -> Self {
        Self {
            schema,
            images: state,
        }
    }
    /// Read presentation names, declared value types and source provenance.
    pub fn bindings(&self) -> &[IoBinding] {
        self.schema.bindings()
    }
    /// Borrow the authoritative physical input image.
    pub fn inputs(&self) -> &[u8] {
        self.images.inputs()
    }
    /// Borrow the authoritative published output image.
    pub fn outputs(&self) -> &[u8] {
        self.images.outputs()
    }
    /// Borrow the authoritative marker image.
    pub fn memory(&self) -> &[u8] {
        self.images.memory()
    }
    /// Read a selected wire value for presentation outside RUN. Only that value
    /// is cloned for hierarchical addresses. Checked engineering reads use the
    /// owner's separately accounted `read_image` operation.
    pub fn read(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        validate_process_image_address(address)?;
        if address.path.len() > 1 {
            return self.images.hierarchical(address).cloned().ok_or_else(|| {
                RuntimeError::InvalidIoAddress(format!("hier {:?}", address.path).into())
            });
        }
        let image = match address.area {
            IoArea::Input => self.inputs(),
            IoArea::Output => self.outputs(),
            IoArea::Memory => self.memory(),
        };
        trust_runtime_core::io_image::read_flat_image(image, address)
    }
    /// Render the authoring-compatible binding snapshot against current images.
    /// This does not clone unused images or re-create a live authoring interface.
    pub fn snapshot(&self) -> IoSnapshot {
        self.schema.snapshot_with(|address| self.read(address))
    }
}
