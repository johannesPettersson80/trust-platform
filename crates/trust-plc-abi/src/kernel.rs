//! What a kernel uses to call a module: the context it fills and the entry points it calls
//! (specification 44 §8.2, §8.3, §10.3).

use core::ptr::NonNull;

use crate::{Area, Cx, EntryFn, FaultRecord, KernelApi, AREA_COUNT, INTERFACE_VERSION};

/// An entry point of an admitted module.
#[derive(Clone, Copy)]
pub struct Entry(EntryFn);

impl Entry {
    /// The entry point whose first instruction is at `address`.
    ///
    /// # Safety
    ///
    /// `address` is the address of an entry point of an image the kernel admitted (specification
    /// 44 §10.2), with the Thumb bit set on a Thumb target as the image's entry table gives it, in
    /// memory the CPU executes, and the module's code and read-only data stay mapped and unchanged
    /// for as long as the entry is called.
    #[must_use]
    #[allow(unsafe_code)]
    pub unsafe fn from_address(address: *const u8) -> Self {
        // SAFETY: as the caller promises, `address` is a function of the module with the signature
        // of `EntryFn`, which every entry point of an image has (specification 44 §5.4).
        Self(unsafe { core::mem::transmute::<*const u8, EntryFn>(address) })
    }

    /// The entry point `function`, a function of this process with the entry signature: a kernel's
    /// own stand-in for a module in its tests.
    #[must_use]
    pub fn from_fn(function: EntryFn) -> Self {
        Self(function)
    }

    /// Calls the entry point with `cx` and returns 0 or the code of the fault it ended with.
    pub fn call(self, cx: &mut Cx) -> u32 {
        (self.0)(cx)
    }
}

/// The base a kernel gives an area of no bytes: never null, aligned to 8.
fn empty_base() -> *mut u8 {
    NonNull::<u64>::dangling().as_ptr().cast::<u8>()
}

impl Cx {
    /// A context of this interface version with the kernel functions `api`, empty areas, no time,
    /// no budget and no breakpoints, which the kernel fills before each call.
    #[must_use]
    pub fn new(api: &'static KernelApi) -> Self {
        Self {
            version: INTERFACE_VERSION,
            task: u32::MAX,
            areas: [Area {
                base: empty_base(),
                len: 0,
            }; AREA_COUNT],
            now_ns: 0,
            budget: 0,
            stmt: 0,
            armed: 0,
            breakpoint_bytes: 0,
            breakpoints: core::ptr::null(),
            api,
            kernel: core::ptr::null_mut(),
            fault: FaultRecord::default(),
        }
    }

    /// Gives area `id` the `len` bytes from `base`; an empty area gets an aligned base that is
    /// never read. Ids outside 1 to 7 change nothing.
    pub fn set_area(&mut self, id: u8, base: *mut u8, len: u32) {
        let Some(area) = usize::from(id)
            .checked_sub(1)
            .and_then(|index| self.areas.get_mut(index))
        else {
            return;
        };
        *area = if len == 0 || base.is_null() {
            Area {
                base: empty_base(),
                len: 0,
            }
        } else {
            Area { base, len }
        };
    }
}

// SAFETY: a context's pointers name memory that the kernel's cycle owns together with the context:
// the areas, the breakpoint bits and the function table. The cycle moves them as one, and a module
// reads them only during a call the cycle makes on its own thread.
#[allow(unsafe_code)]
unsafe impl Send for Cx {}

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn continue_stop(_cx: *mut Cx, _stmt: u32) -> u32 {
    0
}
#[cfg(not(target_arch = "x86_64"))]
extern "C" fn continue_stop(_cx: *mut Cx, _stmt: u32) -> u32 {
    0
}

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn no_queued_write(_cx: *mut Cx, _symbol: u32) -> u32 {
    0
}
#[cfg(not(target_arch = "x86_64"))]
extern "C" fn no_queued_write(_cx: *mut Cx, _symbol: u32) -> u32 {
    0
}

/// The functions of a kernel without a debugger: a stop continues at once and no write is queued.
pub static WITHOUT_DEBUGGER: KernelApi = KernelApi {
    stop: continue_stop,
    debug_write: no_queued_write,
};

/// A module's code and read-only data in memory the CPU executes, on a computer (specification 44
/// §10.3): copied into an anonymous mapping that is then made executable and no longer writable.
#[cfg(feature = "host")]
pub struct Executable {
    map: memmap2::Mmap,
}

#[cfg(feature = "host")]
impl Executable {
    /// The module bytes `module` (`CODE` with `RODATA` at its distance, as the image holds them)
    /// copied into executable memory.
    ///
    /// # Errors
    ///
    /// The error of the operating system's mapping or protection change.
    pub fn load(module: &[u8]) -> std::io::Result<Self> {
        let mut map = memmap2::MmapMut::map_anon(module.len().max(1))?;
        map[..module.len()].copy_from_slice(module);
        Ok(Self {
            map: map.make_exec()?,
        })
    }

    /// The entry point `offset` bytes into the code, or `None` past it. The entry may be called
    /// only while this mapping lives: a kernel keeps both in the same program.
    #[must_use]
    #[allow(unsafe_code)]
    pub fn entry(&self, offset: u32) -> Option<Entry> {
        let offset = usize::try_from(offset)
            .ok()
            .filter(|offset| *offset < self.map.len())?;
        // SAFETY: the mapping holds the code of a module the kernel admitted, executable and
        // unchanged since it was made executable, and `offset` is the offset of one of its entry
        // points from its first byte, inside the mapping.
        Some(unsafe { Entry::from_address(self.map.as_ptr().add(offset)) })
    }
}
