//! The records of a debug image's Rust POU calls (specification 44 §5.13, §8.2; specification 48
//! RU-49): the program makes them from the image's Rust POU table when it is built, and the module
//! keeps each POU's longest call and its calls over its budget through every activation's context.

use trust_plc_abi::RustTime;

use super::super::image::RustPouRecord;
use super::{ModuleCode, NativeProgram};

impl<C: ModuleCode> NativeProgram<'_, C> {
    /// The record of each Rust POU of a debug image, in the order of its Rust POU table; none for
    /// an image without the table, as a release image is.
    #[must_use]
    pub fn rust_times(&self) -> &[RustTime] {
        &self.rust_times
    }

    /// The name of the Rust POU numbered `pou` in the image's Rust POU table, as Structured Text
    /// declares it.
    #[must_use]
    pub fn rust_pou_name(&self, pou: u32) -> Option<&str> {
        let image = self.image();
        let name = image.table::<RustPouRecord>().ok()?.get(pou)?.name;
        image.string(name)
    }
}
