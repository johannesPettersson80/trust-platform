//! The program image window (specification 34 MEM-14, the program image in place): the C6's MMU maps
//! the flash pages that hold an installed container into the cache's address space, above the
//! firmware's own pages, so that the runtime reads the program where it lies in its slot.
//!
//! The MMU maps 256 pages of 64 KiB from `0x4200_0000`. The firmware's code and read-only data take
//! the first entries, and the bootloader keeps the last entry for its own flash reads, as esp-hal
//! leaves it when it maps PSRAM on the chips that have it. The window takes the entries after the
//! highest one the firmware maps, found once, before this module writes any entry. A mapping
//! replaces the window's earlier one, which the runtime no longer reads by then
//! (`ProgramFlash::map`).

use core::sync::atomic::{AtomicU32, Ordering};

use esp_hal::peripherals::SPI0;

/// The bytes of one MMU page.
const PAGE_BYTES: u32 = 0x1_0000;

/// The MMU's entries.
const ENTRIES: u32 = 256;

/// The address that entry 0 maps.
const ORIGIN: u32 = 0x4200_0000;

/// The ROM's `Cache_Invalidate_Addr(addr, size)`, which drops the cache lines of `size` bytes from
/// `addr` (`esp32c6.rom.ld` of esp-rom-sys).
const CACHE_INVALIDATE_ADDR: usize = 0x4000_0648;

/// The window's first entry, 0 until it is found (entry 0 is always the firmware's).
static WINDOW: AtomicU32 = AtomicU32::new(0);

/// The `len` bytes of flash from `address` where the window maps them, or `None` when they do not
/// fit the window.
pub(super) fn map(address: u32, len: u32) -> Option<&'static [u8]> {
    let end = address.checked_add(len)?.checked_sub(1)?;
    let first_page = address / PAGE_BYTES;
    let pages = end / PAGE_BYTES - first_page + 1;
    let window = window();
    if window + pages >= ENTRIES {
        return None;
    }
    for page in 0..pages {
        let physical = u16::try_from(first_page + page).ok()?;
        select(window + page);
        // A valid entry of a flash page, not decrypted (the C6's flash is not encrypted).
        SPI0::regs().mmu_item_content().write(|w| {
            unsafe { w.paddr().bits(physical) };
            w.valid().set_bit()
        });
    }
    let start = ORIGIN + window * PAGE_BYTES;
    // SAFETY: a call of the ROM's Cache_Invalidate_Addr with the C ABI: a0 and a1 carry the window's
    // range, and every register the ABI lets a callee change is clobbered. The function only drops
    // the cache lines of that range, which an earlier mapping of the window may have filled.
    unsafe {
        core::arch::asm!(
            "jalr ra, 0({function})",
            function = in(reg) CACHE_INVALIDATE_ADDR,
            in("a0") start,
            in("a1") pages * PAGE_BYTES,
            clobber_abi("C"),
        );
    }
    let bytes = start + address % PAGE_BYTES;
    // SAFETY: the entries written above map the `len` flash bytes from `address` at `bytes`, readable
    // through the cache, and nothing writes them while they are read: the program store writes no
    // slot whose image a held program reads, and the window is mapped again only after the program
    // that read it is dropped (specification 34 §5.1 and MEM-14; `ProgramFlash::map`).
    Some(unsafe { core::slice::from_raw_parts(bytes as *const u8, len as usize) })
}

/// The window's first entry: the entry after the highest one that the firmware maps, below the
/// bootloader's last entry, found at the first call, before any entry of the window is written.
fn window() -> u32 {
    let found = WINDOW.load(Ordering::Relaxed);
    if found != 0 {
        return found;
    }
    let first = (1..ENTRIES - 1)
        .rev()
        .find(|&entry| {
            select(entry);
            SPI0::regs().mmu_item_content().read().valid().bit()
        })
        .map_or(1, |highest| highest + 1);
    WINDOW.store(first, Ordering::Relaxed);
    first
}

/// Selects `entry` for the next read or write of an entry's content.
fn select(entry: u32) {
    SPI0::regs()
        .mmu_item_index()
        .write(|w| unsafe { w.mmu_item_index().bits(entry) });
}
