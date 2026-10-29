//! The instruction cache made coherent with a module copied into memory, before its code runs on a
//! computer (specification 44 §10.3). Its unsafe code is registered for the whole file: the
//! declaration of Apple's `sys_icache_invalidate` and the cache maintenance by address on ARM Linux.

/// Makes the instructions just copied into `code` the ones the CPU fetches there (specification 44
/// §10.3). An ARM CPU fetches instructions through a cache of its own that does not see what was
/// written through the data cache, so without this it may run what that cache still holds for
/// those addresses. On macOS this is Apple's `sys_icache_invalidate`; on Linux it is the
/// architecture's sequence: clean each data cache line to the point of unification, invalidate each
/// instruction cache line, then synchronize. An x86 CPU's fetches see every write, and no other CPU
/// runs a module on a computer (§4.4).
#[allow(unsafe_code)]
pub(super) fn sync_instruction_cache(code: &[u8]) {
    #[cfg(all(target_arch = "aarch64", target_vendor = "apple"))]
    {
        unsafe extern "C" {
            fn sys_icache_invalidate(start: *mut core::ffi::c_void, len: usize);
        }
        // SAFETY: the range is the caller's own mapping, readable and writable for the call, and
        // the function only maintains the caches for it.
        unsafe { sys_icache_invalidate(code.as_ptr().cast_mut().cast(), code.len()) };
    }
    #[cfg(all(target_arch = "aarch64", not(target_vendor = "apple")))]
    {
        let start = code.as_ptr() as usize;
        let end = start + code.len();
        // SAFETY: Linux permits reading CTR_EL0 and cache maintenance by address at EL0, and every
        // address lies in the caller's own readable mapping; the instructions change no memory.
        unsafe {
            let ctr: u64;
            core::arch::asm!("mrs {}, ctr_el0", out(reg) ctr, options(nomem, nostack, preserves_flags));
            let data_line = 4_usize << ((ctr >> 16) & 0xf);
            let instruction_line = 4_usize << (ctr & 0xf);
            let mut line = start & !(data_line - 1);
            while line < end {
                core::arch::asm!("dc cvau, {}", in(reg) line, options(nostack, preserves_flags));
                line += data_line;
            }
            core::arch::asm!("dsb ish", options(nostack, preserves_flags));
            let mut line = start & !(instruction_line - 1);
            while line < end {
                core::arch::asm!("ic ivau, {}", in(reg) line, options(nostack, preserves_flags));
                line += instruction_line;
            }
            core::arch::asm!("dsb ish", "isb", options(nostack, preserves_flags));
        }
    }
    #[cfg(not(target_arch = "aarch64"))]
    let _ = code;
}
