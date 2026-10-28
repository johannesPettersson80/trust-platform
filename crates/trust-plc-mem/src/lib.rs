//! The memory functions of a compiled program's module for a computer (specification 44 §4.5):
//! `memcpy`, `memmove`, `memset`, `memcmp` and `bcmp`, which the compiler calls for a large copy,
//! fill or comparison, such as a long string's or a large array's, and which a module has nowhere
//! else, since it links no C library. A module for a chip takes them from its target's builtins.
//!
//! The crate takes part in the module's link-time optimization, so that every call of these
//! functions is a direct call and the module needs no global offset table. Each loop reads and
//! writes its bytes as volatile accesses, which the compiler never turns back into a call of the
//! function the loop defines.

#![no_std]
// These are the runtime symbols the compiler calls, defined here on purpose for a module that has
// no C library (specification 44 §4.5).
#![allow(suspicious_runtime_symbol_definitions)]

/// Copies `n` bytes from `src` to `dest`, which do not overlap.
///
/// # Safety
///
/// `src` is valid for reading and `dest` for writing `n` bytes, and the two ranges do not overlap.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut index = 0;
    while index < n {
        let byte = unsafe { src.add(index).read_volatile() };
        unsafe { dest.add(index).write_volatile(byte) };
        index += 1;
    }
    dest
}

/// Copies `n` bytes from `src` to `dest`, which may overlap.
///
/// # Safety
///
/// `src` is valid for reading and `dest` for writing `n` bytes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.cast_const() < src {
        let mut index = 0;
        while index < n {
            let byte = unsafe { src.add(index).read_volatile() };
            unsafe { dest.add(index).write_volatile(byte) };
            index += 1;
        }
    } else {
        let mut index = n;
        while index > 0 {
            index -= 1;
            let byte = unsafe { src.add(index).read_volatile() };
            unsafe { dest.add(index).write_volatile(byte) };
        }
    }
    dest
}

/// Sets `n` bytes at `dest` to the low byte of `value`.
///
/// # Safety
///
/// `dest` is valid for writing `n` bytes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut u8, value: i32, n: usize) -> *mut u8 {
    let byte = value.to_le_bytes()[0];
    let mut index = 0;
    while index < n {
        unsafe { dest.add(index).write_volatile(byte) };
        index += 1;
    }
    dest
}

/// Compares `n` bytes at `a` and `b`: the difference of the first unequal pair, read unsigned, or
/// 0.
///
/// # Safety
///
/// `a` and `b` are valid for reading `n` bytes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut index = 0;
    while index < n {
        let (left, right) = unsafe { (a.add(index).read_volatile(), b.add(index).read_volatile()) };
        if left != right {
            return i32::from(left) - i32::from(right);
        }
        index += 1;
    }
    0
}

/// Whether `n` bytes at `a` and `b` differ: 0 when they are equal.
///
/// # Safety
///
/// `a` and `b` are valid for reading `n` bytes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut index = 0;
    while index < n {
        let (left, right) = unsafe { (a.add(index).read_volatile(), b.add(index).read_volatile()) };
        if left != right {
            return 1;
        }
        index += 1;
    }
    0
}
