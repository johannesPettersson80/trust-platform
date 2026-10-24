//! The image of an online change in the heap (specification 44 §12.3): a change writes no flash, so
//! the new image stays where its upload lies for as long as its program runs. The program's code
//! owns it ([`ModuleMemory::place_change`](super::ModuleMemory::place_change)), so it is freed with
//! the program.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr::NonNull;

/// An image in the heap, freed when it is dropped.
pub struct ChangedImage(NonNull<[u8]>);

impl ChangedImage {
    /// The image of `upload`, in the heap block it keeps.
    #[must_use]
    pub fn new(upload: Vec<u8>) -> Self {
        Self(NonNull::from(Box::leak(upload.into_boxed_slice())))
    }

    /// The image's bytes, for the one program built from them.
    ///
    /// # Safety
    ///
    /// The caller builds one program from the bytes and gives that program this image to own, in
    /// its code, so that the bytes are freed with the program and not before; nothing else keeps
    /// them.
    #[must_use]
    pub unsafe fn bytes(&self) -> &'static [u8] {
        // SAFETY: the block is this image's own from `new` until `drop`, and nothing writes it; the
        // caller keeps the image alive for as long as the bytes are used.
        unsafe { self.0.as_ref() }
    }
}

impl Drop for ChangedImage {
    fn drop(&mut self) {
        // SAFETY: the block came from `Box::leak` in `new` and is freed once, here.
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}
