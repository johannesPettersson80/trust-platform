//! The program code region (specification 44 §10.3): RAM that the C6 executes, into which the loader
//! copies the module of the program that runs.
//!
//! From the image window the module runs through the C6's 32 KiB flash cache, which it shares with
//! the firmware: the pasteurizer's control task took 0.243 ms on average and 1.05 ms at worst from
//! flash, and 0.063 ms and 0.178 ms over an hour from RAM (22 October 2026). The region is the linker's
//! `.trust_code` section (`firmware/trust-esp32c6/trust_code.x`), between the static data and the
//! stack, where neither of esp-hal's no-execute ranges reaches, so the CPU executes what is copied
//! into it. One program holds the region at a time: the loop drops the program that ran before it
//! loads the next one (specification 34 STATE-08), and an online change copies the new program's
//! module over the running one at the boundary, where no scan runs, and copies the running one back
//! when the change does not complete (specification 44 §12.3). A module larger than the region runs
//! where it lies in the slot; an online change, whose image lies in the heap, which the C6 does not
//! execute, refuses it.

mod region;

use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicU32, Ordering};

use trust_plc_abi::kernel::{Entry, InPlace};
use trust_runtime_core::native::program::ModuleCode;

use crate::compiled_loader::{ChangedImage, ModuleMemory};
use crate::profile::PROGRAM_CODE_BYTES;
use region::REGION;

/// The number of the program whose module holds the region; 0 when none does.
static HOLDER: AtomicU32 = AtomicU32::new(0);

/// The number the next program that takes the region gets.
static NEXT_HOLDER: AtomicU32 = AtomicU32::new(1);

/// The C6's module memory: the program code region, and the slot for a module that does not fit it.
pub struct ProgramCode;

impl ModuleMemory for ProgramCode {
    type Code = C6Code;

    /// Copies `module` into the program code region when it fits and the region is free, and
    /// otherwise runs it where it lies in the mapped slot.
    fn place(&mut self, module: &'static [u8]) -> C6Code {
        if module.len() <= PROGRAM_CODE_BYTES && HOLDER.load(Ordering::Acquire) == 0 {
            return C6Code::Ram {
                len: module.len(),
                holder: take_region(module),
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

    /// Copies the module of an online change into the program code region over the running
    /// program's module (specification 44 §12.3, step 4); the code owns the heap `image` the module
    /// lies in.
    fn place_change(
        &mut self,
        module: &'static [u8],
        image: ChangedImage,
    ) -> Result<C6Code, String> {
        if module.len() > PROGRAM_CODE_BYTES {
            return Err(format!(
                "the module of {} bytes does not fit the program code region of \
                 {PROGRAM_CODE_BYTES} bytes",
                module.len()
            ));
        }
        Ok(C6Code::Ram {
            len: module.len(),
            holder: take_region(module),
            image: Some(image),
        })
    }

    /// Copies the module of `running` back into the region, which it holds again, after a change
    /// that did not complete; a running program that runs from its slot had no module there.
    fn restore(&mut self, running: &C6Code, module: &'static [u8]) {
        if let C6Code::Ram { holder, .. } = running {
            write_region(module);
            HOLDER.store(*holder, Ordering::Release);
        }
    }
}

/// Copies `module` into the region for a program that takes it, and returns the number it holds
/// the region under.
fn take_region(module: &[u8]) -> u32 {
    write_region(module);
    let holder = NEXT_HOLDER.fetch_add(1, Ordering::Relaxed);
    HOLDER.store(holder, Ordering::Release);
    holder
}

/// Writes `module`, at most the region's bytes, at the start of the region, and makes the CPU fetch
/// the written bytes.
fn write_region(module: &[u8]) {
    let base = core::ptr::addr_of_mut!(REGION).cast::<u8>();
    let len = module.len().min(PROGRAM_CODE_BYTES);
    // SAFETY: the region holds `PROGRAM_CODE_BYTES` bytes and at most that many are written. The
    // loop writes it before a program's first scan, or at a boundary, where no scan runs, so no code
    // in the region runs or is read while it is written; `module` lies in flash or in the heap,
    // apart from the region. The instruction fence makes the CPU fetch the written bytes and not
    // what it may have read before.
    unsafe {
        core::ptr::copy_nonoverlapping(module.as_ptr(), base, len);
        core::arch::asm!("fence.i");
    }
}

/// Where the module of the program that runs lies.
pub enum C6Code {
    /// The first `len` bytes of the program code region, which this program holds under the number
    /// `holder` until it is dropped or an online change takes the region over.
    Ram {
        /// The module's bytes.
        len: usize,
        /// The number the program holds the region under.
        holder: u32,
        /// The heap image an online change brought, which the program reads for as long as it
        /// runs (specification 44 §12.3); `None` for an image in its slot.
        image: Option<ChangedImage>,
    },
    /// Where the image lies in the mapped slot.
    Flash(InPlace),
}

impl ModuleCode for C6Code {
    fn entry(&self, offset: u32) -> Option<Entry> {
        match self {
            Self::Ram { len, .. } => {
                let offset = usize::try_from(offset).ok().filter(|offset| offset < len)?;
                let base = core::ptr::addr_of!(REGION).cast::<u8>();
                // SAFETY: the region holds the module this program copied into it, executable and
                // unchanged while the program runs: an online change replaces it only at a
                // boundary, after which this program is dropped without running again, or is copied
                // back before it runs again; `offset` is the offset of one of the module's entry
                // points, inside the module.
                Some(unsafe { Entry::from_address(base.wrapping_add(offset)) })
            }
            Self::Flash(code) => code.entry(offset),
        }
    }
}

impl Drop for C6Code {
    fn drop(&mut self) {
        if let Self::Ram { holder, .. } = self {
            // A program whose region an online change took over leaves it to the new program.
            let _ = HOLDER.compare_exchange(*holder, 0, Ordering::AcqRel, Ordering::Relaxed);
        }
    }
}
