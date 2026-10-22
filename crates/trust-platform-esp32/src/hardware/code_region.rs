//! The program code region (specification 44 §10.3): RAM that the C6 executes, into which the loader
//! copies the module of the program that runs.
//!
//! From the image window the module runs through the C6's 32 KiB flash cache, which it shares with
//! the firmware: the pasteurizer's control task took 0.243 ms on average and 1.05 ms at worst from
//! flash, and 0.063 ms and 0.178 ms over an hour from RAM (22 October 2026). The region is the linker's
//! `.trust_code` section (`firmware/trust-esp32c6/trust_code.x`), between the static data and the
//! stack, where neither of esp-hal's no-execute ranges reaches, so the CPU executes what is copied
//! into it. One program holds the region at a time: the loop drops the program that ran before it
//! loads the next one (specification 34 STATE-08). A module larger than the region runs where it
//! lies in the slot.

mod region;

use core::sync::atomic::{AtomicBool, Ordering};

use trust_plc_abi::kernel::{Entry, InPlace};
use trust_runtime_core::native::program::ModuleCode;

use crate::compiled_loader::ModuleMemory;
use crate::profile::PROGRAM_CODE_BYTES;
use region::REGION;

/// Whether a program's module holds the region.
static HELD: AtomicBool = AtomicBool::new(false);

/// The C6's module memory: the program code region, and the slot for a module that does not fit it.
pub struct ProgramCode;

impl ModuleMemory for ProgramCode {
    type Code = C6Code;

    /// Copies `module` into the program code region when it fits and the region is free, and
    /// otherwise runs it where it lies in the mapped slot.
    fn place(&mut self, module: &'static [u8]) -> C6Code {
        if module.len() <= PROGRAM_CODE_BYTES && !HELD.swap(true, Ordering::Acquire) {
            let base = core::ptr::addr_of_mut!(REGION).cast::<u8>();
            // SAFETY: the region holds `PROGRAM_CODE_BYTES` bytes and `module` is no longer; no
            // program holds the region (`HELD` was false and is true now), so nothing reads or runs
            // it while it is written, and `module` lies in flash, apart from it. The instruction
            // fence makes the CPU fetch the written bytes and not what it may have read before.
            unsafe {
                core::ptr::copy_nonoverlapping(module.as_ptr(), base, module.len());
                core::arch::asm!("fence.i");
            }
            return C6Code::Ram { len: module.len() };
        }
        // SAFETY: `module` is the code and read-only data of an admitted image in the slot that the
        // image window maps from flash, which the C6 executes in place: esp-hal maps its own code
        // through the same window, and its memory protection covers only RAM. The program store
        // writes no slot whose program runs, and the window is mapped again only after the program
        // that runs from it is dropped (specification 34 §5.1 and MEM-14), so the module stays
        // mapped and unchanged while the cycle that calls it lives.
        C6Code::Flash(unsafe { InPlace::new(module) })
    }
}

/// Where the module of the program that runs lies.
pub enum C6Code {
    /// The first `len` bytes of the program code region, which this program holds until it is
    /// dropped.
    Ram {
        /// The module's bytes.
        len: usize,
    },
    /// Where the image lies in the mapped slot.
    Flash(InPlace),
}

impl ModuleCode for C6Code {
    fn entry(&self, offset: u32) -> Option<Entry> {
        match self {
            Self::Ram { len } => {
                let offset = usize::try_from(offset).ok().filter(|offset| offset < len)?;
                let base = core::ptr::addr_of!(REGION).cast::<u8>();
                // SAFETY: the region holds the module this program copied into it, which it holds
                // until it is dropped, executable and unchanged since the copy, and `offset` is the
                // offset of one of the module's entry points, inside the module.
                Some(unsafe { Entry::from_address(base.wrapping_add(offset)) })
            }
            Self::Flash(code) => code.entry(offset),
        }
    }
}

impl Drop for C6Code {
    fn drop(&mut self) {
        if matches!(self, Self::Ram { .. }) {
            HELD.store(false, Ordering::Release);
        }
    }
}
