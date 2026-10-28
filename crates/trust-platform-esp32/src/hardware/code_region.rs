//! The program region (specification 44 §10.3, §12.3): RAM that the C6 executes, which holds the
//! code of the programs that run.
//!
//! From the image window the module runs through the C6's 32 KiB flash cache, which it shares with
//! the firmware: the pasteurizer's control task took 0.243 ms on average and 1.05 ms at worst from
//! flash, and 0.063 ms and 0.178 ms over an hour from RAM (22 October 2026). The region is the
//! linker's `.trust_code` section (`firmware/trust-esp32c6/trust_code.x`), between the static data
//! and the stack, where neither of esp-hal's no-execute ranges reaches, so the CPU executes what
//! lies in it. The firmware gives it to its heap as the heap's program region
//! (`MeasuredHeap::init_program_region`): the installed program's module is copied into a block
//! there, which the heap places at the top, and an online change's image, which the transfer
//! uploads into a block the heap places at the bottom, runs its module where it lies, beside the
//! running program's. A module the region has no room for runs where it lies in the slot; an online
//! change's, whose image lies in the heap, which the C6 does not execute, is refused.

mod region;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use trust_plc_abi::kernel::{Entry, InPlace};
use trust_runtime_core::native::program::ModuleCode;

use crate::compiled_loader::{ChangedImage, ModuleMemory};
use crate::profile::PROGRAM_CODE_BYTES;
use region::REGION;

/// The program region's bytes, which the firmware gives to its heap.
///
/// # Safety
///
/// Call once, before the heap allocates: the firmware hands the slice to its heap's program region,
/// and nothing else reads or writes it but through the heap's blocks.
#[must_use]
pub unsafe fn program_region() -> &'static mut [u8] {
    // SAFETY: `REGION` is the `.trust_code` section's one static of `PROGRAM_CODE_BYTES` bytes, and
    // the caller hands this one slice of it to the heap, which owns it from then on.
    unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(REGION).cast::<u8>(),
            PROGRAM_CODE_BYTES,
        )
    }
}

/// Whether the `len` bytes at `address` lie in the program region, which the CPU executes.
fn executable(address: usize, len: usize) -> bool {
    let start = core::ptr::addr_of!(REGION) as usize;
    address >= start
        && address
            .checked_add(len)
            .is_some_and(|end| end <= start + PROGRAM_CODE_BYTES)
}

/// Makes the CPU fetch the bytes written into the program region and not what it may have read
/// before.
fn fetch_written() {
    // SAFETY: `fence.i` orders instruction fetches after the earlier stores (RISC-V Zifencei); it
    // touches no memory and no register.
    unsafe { core::arch::asm!("fence.i") };
}

/// One 64-byte line of a module copy. A block of lines is asked with the module's alignment, which
/// the heap places at the top of its program region.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Line([u8; 64]);

/// The copy of `module` in a block of the program region, or `None` when the region has no room
/// for it.
fn copy_into_region(module: &[u8]) -> Option<Vec<Line>> {
    let lines = module.len().div_ceil(64);
    let mut copy: Vec<Line> = Vec::new();
    copy.try_reserve_exact(lines).ok()?;
    // A heap with no room in its program region gives a block of a data region, which the C6 does
    // not execute.
    if !executable(copy.as_ptr() as usize, lines * 64) {
        return None;
    }
    for chunk in module.chunks(64) {
        let mut line = Line([0; 64]);
        line.0[..chunk.len()].copy_from_slice(chunk);
        copy.push(line);
    }
    fetch_written();
    Some(copy)
}

/// The C6's module memory: the program region, and the slot for a module that does not fit it.
pub struct ProgramCode;

impl ModuleMemory for ProgramCode {
    type Code = C6Code;

    /// Copies `module` into a block of the program region when the region has room for it, and
    /// otherwise runs it where it lies in the mapped slot.
    fn place(&mut self, module: &'static [u8]) -> C6Code {
        if let Some(copy) = copy_into_region(module) {
            return C6Code::Ram {
                copy,
                len: module.len(),
                image: None,
            };
        }
        // SAFETY: `module` is the code and read-only data of an admitted image in the slot that the
        // image window maps from flash, which the C6 executes in place: esp-hal maps its own code
        // through the same window, and its memory protection covers only RAM. The program store
        // writes no slot whose program runs, and the window is mapped again only after the program
        // that runs from it is dropped (specification 34 §5.1 and MEM-14), so the module stays
        // mapped and unchanged while the cycle that calls it lives.
        C6Code::Flash(unsafe { InPlace::new(module) })
    }

    /// Runs the module of an online change where it lies in its image, when the transfer uploaded
    /// the image into the program region, and otherwise copies it into a block there; the running
    /// program's module stays where it is (specification 44 §12.3, step 4).
    fn place_change(
        &mut self,
        module: &'static [u8],
        image: ChangedImage,
    ) -> Result<C6Code, String> {
        if executable(module.as_ptr() as usize, module.len()) {
            fetch_written();
            return Ok(C6Code::Image { module, image });
        }
        let copy = copy_into_region(module).ok_or_else(|| {
            format!(
                "insufficient memory: the program memory of {PROGRAM_CODE_BYTES} bytes has no room \
                 for the change's module of {} bytes beside the running program's",
                module.len()
            )
        })?;
        Ok(C6Code::Ram {
            copy,
            len: module.len(),
            image: Some(image),
        })
    }
}

/// Where the module of the program that runs lies.
pub enum C6Code {
    /// A copy of the module in a block of the program region, which the program owns, with the
    /// image of an online change whose module the transfer could not upload into the region.
    Ram {
        /// The copy.
        copy: Vec<Line>,
        /// The module's bytes.
        len: usize,
        /// The image an online change brought, which the program reads for as long as it runs.
        image: Option<ChangedImage>,
    },
    /// The module of an online change's image, where it lies in the program region (specification
    /// 44 §12.3).
    Image {
        /// The module, which lies in `image`.
        module: &'static [u8],
        /// The image, which the program owns and reads for as long as it runs.
        image: ChangedImage,
    },
    /// Where the image lies in the mapped slot.
    Flash(InPlace),
}

impl ModuleCode for C6Code {
    fn entry(&self, offset: u32) -> Option<Entry> {
        let (base, len): (*const u8, usize) = match self {
            Self::Ram { copy, len, .. } => (copy.as_ptr().cast(), *len),
            Self::Image { module, .. } => (module.as_ptr(), module.len()),
            Self::Flash(code) => return code.entry(offset),
        };
        let offset = usize::try_from(offset)
            .ok()
            .filter(|offset| *offset < len)?;
        // SAFETY: the module lies at `base` in the program region, which the CPU executes, in a
        // block this code owns (the copy, or the image), unchanged since `fetch_written`, for as
        // long as the program lives; `offset` is the offset of one of its entry points, inside it.
        Some(unsafe { Entry::from_address(base.wrapping_add(offset)) })
    }
}
