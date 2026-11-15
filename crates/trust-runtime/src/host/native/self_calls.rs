//! A function's direct call of itself (specification 48 RU-18). The assembler resolves it inside
//! the function's own section, so the call relocations the call graph reads leave it out
//! (specification 44 §6.4); its call instruction shows it.

use trust_runtime_core::native::image::Target;

/// Whether the code `bytes` of a function, whose start is its first byte, holds a direct call of
/// that start: x86-64's `call`, AArch64's and Thumb's `bl`, RISC-V's `jal`, `c.jal` and `auipc`
/// with `jalr`, each with a link register. A jump back to the start, a loop, is no call.
pub(super) fn calls_itself(bytes: &[u8], target: Target) -> bool {
    match target {
        Target::X86_64 => x86_64(bytes),
        Target::Aarch64 => (0..bytes.len()).step_by(4).any(|at| {
            word(bytes, at).is_some_and(|word| {
                word >> 26 == 0b10_0101 && lands(at, sign_extend(word & 0x03FF_FFFF, 26) * 4)
            })
        }),
        // A function's constant pool lies inside its code, so every halfword is read as a possible
        // `bl`; only one whose offset leads exactly to the start counts.
        Target::ThumbV8mMainHf => (0..bytes.len()).step_by(2).any(|at| {
            half(bytes, at)
                .zip(half(bytes, at + 2))
                .is_some_and(|(first, second)| thumb_call_of(at, first, second))
        }),
        Target::Rv32imac => risc_v(bytes),
    }
}

/// x86-64: the instructions are read one after the other from the start, so that a byte inside
/// one, a displacement or an immediate of `0xE8`, is never taken for a call. From an instruction
/// the decoder does not know on, every byte is read as a possible call, so that none is missed.
fn x86_64(bytes: &[u8]) -> bool {
    let mut at = 0;
    while at < bytes.len() {
        let Some((length, call)) = x86_64_instruction(&bytes[at..]) else {
            return (at..bytes.len()).any(|at| {
                bytes[at] == 0xE8
                    && word(bytes, at + 1).is_some_and(|word| lands(at + 5, sign_extend(word, 32)))
            });
        };
        if call.is_some_and(|displacement| lands(at + length, displacement)) {
            return true;
        }
        at += length;
    }
    false
}

/// The length of the x86-64 instruction that `code` starts with and, for a direct `call`, its
/// displacement, which counts from the instruction's end; `None` for one the decoder does not know
/// or that `code` cuts short. It reads 64-bit mode's legacy prefixes, REX, the one-, two- and
/// three-byte opcode maps, VEX, EVEX and XOP.
fn x86_64_instruction(code: &[u8]) -> Option<(usize, Option<i64>)> {
    let mut at = 0;
    let (mut operand16, mut address32) = (false, false);
    loop {
        match *code.get(at)? {
            0x66 => operand16 = true,
            0x67 => address32 = true,
            0x26 | 0x2E | 0x36 | 0x3E | 0x64 | 0x65 | 0xF0 | 0xF2 | 0xF3 => {}
            _ => break,
        }
        at += 1;
    }
    let rex = (*code.get(at)? & 0xF0) == 0x40;
    let wide = rex && (code[at] & 0x08) != 0;
    if rex {
        at += 1;
    }
    let opcode = *code.get(at)?;
    // An immediate of the operand size: 16 or 32 bits.
    let z = if operand16 { 2 } else { 4 };
    // The opcode's bytes, whether a ModRM byte follows them, and the immediate's bytes.
    let (opcode_bytes, modrm, immediate) = match opcode {
        0x0F => match *code.get(at + 1)? {
            0x38 => (3, true, 0),
            0x3A => (3, true, 1),
            // 3DNow!, whose opcode follows the operands as a byte.
            0x0F => (2, true, 1),
            0x05..=0x09
            | 0x0B
            | 0x0E
            | 0x30..=0x35
            | 0x37
            | 0x77
            | 0xA0..=0xA2
            | 0xA8..=0xAA
            | 0xC8..=0xCF => (2, false, 0),
            0x80..=0x8F => (2, false, 4),
            0x70..=0x73 | 0xA4 | 0xAC | 0xBA | 0xC2 | 0xC4..=0xC6 => (2, true, 1),
            0x04 | 0x0A | 0x0C | 0x24..=0x27 | 0x36 | 0x39 | 0x3B..=0x3F | 0x7A | 0x7B => {
                return None;
            }
            // The rest, VIA PadLock's `0F A6` and `0F A7` among them, have a ModRM byte.
            _ => (2, true, 0),
        },
        0xC5 => {
            let (modrm, immediate) = vector(1, *code.get(at + 2)?)?;
            (3, modrm, immediate)
        }
        0xC4 => {
            let (modrm, immediate) = vector(code.get(at + 1)? & 0x1F, *code.get(at + 3)?)?;
            (4, modrm, immediate)
        }
        0x62 => {
            let (modrm, immediate) = vector(code.get(at + 1)? & 0x07, *code.get(at + 4)?)?;
            (5, modrm, immediate)
        }
        // XOP, where `pop` would name a register field of 0 and so a map below 8.
        0x8F if (code.get(at + 1)? & 0x1F) >= 8 => match code[at + 1] & 0x1F {
            0x08 => (4, true, 1),
            0x09 => (4, true, 0),
            0x0A => (4, true, 4),
            _ => return None,
        },
        0x00..=0x3F => match opcode & 7 {
            0..=3 => (1, true, 0),
            4 => (1, false, 1),
            5 => (1, false, z),
            // The segment pushes and decimal adjustments, which 64-bit mode does not have.
            _ => return None,
        },
        0x50..=0x5F
        | 0x6C..=0x6F
        | 0x90..=0x99
        | 0x9B..=0x9F
        | 0xA4..=0xA7
        | 0xAA..=0xAF
        | 0xC3
        | 0xC9
        | 0xCB
        | 0xCC
        | 0xCF
        | 0xD7
        | 0xEC..=0xEF
        | 0xF1
        | 0xF4
        | 0xF5
        | 0xF8..=0xFD => (1, false, 0),
        0x63 | 0x84..=0x8F | 0xD0..=0xD3 | 0xD8..=0xDF | 0xFE | 0xFF => (1, true, 0),
        0x68 | 0xA9 => (1, false, z),
        0x69 | 0x81 | 0xC7 => (1, true, z),
        0x6A | 0x70..=0x7F | 0xA8 | 0xB0..=0xB7 | 0xCD | 0xE0..=0xE7 | 0xEB => (1, false, 1),
        0x6B | 0x80 | 0x83 | 0xC0 | 0xC1 | 0xC6 => (1, true, 1),
        0xA0..=0xA3 => (1, false, if address32 { 4 } else { 8 }),
        0xB8..=0xBF => (1, false, if wide { 8 } else { z }),
        0xC2 | 0xCA => (1, false, 2),
        0xC8 => (1, false, 3),
        0xE8 | 0xE9 => (1, false, 4),
        // Group 3: `test` (/0, /1) takes an immediate, the others none.
        0xF6 | 0xF7 => {
            let test = (code.get(at + 1)? >> 3) & 7 <= 1;
            let immediate = match (test, opcode) {
                (false, _) => 0,
                (true, 0xF6) => 1,
                (true, _) => z,
            };
            (1, true, immediate)
        }
        _ => return None,
    };
    let mut length = at + opcode_bytes;
    if modrm {
        length += modrm_length(code.get(length..)?)?;
    }
    length += immediate;
    if length > code.len().min(15) {
        return None;
    }
    let call = if opcode == 0xE8 {
        Some(sign_extend(word(code, length - 4)?, 32))
    } else {
        None
    };
    Some((length, call))
}

/// Whether a VEX, EVEX or XOP instruction of the opcode map `map` with the opcode `opcode` has a
/// ModRM byte, and its immediate's bytes; `None` for a map the decoder does not know.
fn vector(map: u8, opcode: u8) -> Option<(bool, usize)> {
    match map {
        // `vzeroupper` and `vzeroall` have no operands.
        1 => Some((
            opcode != 0x77,
            usize::from(matches!(opcode, 0x70..=0x73 | 0xC2 | 0xC4..=0xC6)),
        )),
        2 | 5 | 6 => Some((true, 0)),
        3 => Some((true, 1)),
        _ => None,
    }
}

/// The bytes of the ModRM byte that `code` starts with and of the SIB byte and the displacement
/// it names.
fn modrm_length(code: &[u8]) -> Option<usize> {
    let modrm = *code.first()?;
    let (mode, rm) = (modrm >> 6, modrm & 7);
    if mode == 3 {
        return Some(1);
    }
    let (sib, base) = if rm == 4 {
        (1, code.get(1)? & 7)
    } else {
        (0, rm)
    };
    // Mode 0 with base 5 is a 32-bit displacement: from the next instruction without a SIB byte,
    // from no base with one.
    let displacement = match mode {
        0 if base == 5 => 4,
        0 => 0,
        1 => 1,
        _ => 4,
    };
    Some(1 + sib + displacement)
}

/// Whether the halfwords `first` and `second` at `at` are Thumb's `bl` of the start.
fn thumb_call_of(at: usize, first: u16, second: u16) -> bool {
    if first >> 11 != 0b1_1110 || second >> 14 != 0b11 || (second >> 12) & 1 != 1 {
        return false;
    }
    let sign = u32::from((first >> 10) & 1);
    let i1 = !(u32::from((second >> 13) & 1) ^ sign) & 1;
    let i2 = !(u32::from((second >> 11) & 1) ^ sign) & 1;
    let offset = (sign << 24)
        | (i1 << 23)
        | (i2 << 22)
        | (u32::from(first & 0x3FF) << 12)
        | (u32::from(second & 0x7FF) << 1);
    // The offset counts from the instruction's address and 4.
    lands(at + 4, sign_extend(offset, 25))
}

/// RISC-V: the instructions are read one after the other from the start, their length from their
/// two lowest bits, so that the upper half of a 32-bit instruction is never taken for a `c.jal`.
/// A link register is `ra` or `t0`.
fn risc_v(bytes: &[u8]) -> bool {
    let link = |register: u32| register == 1 || register == 5;
    let mut at = 0;
    while let Some(first) = half(bytes, at) {
        if first & 0b11 != 0b11 {
            // `c.jal`, RV32C's call with `ra`.
            if first & 0xE003 == 0x2001 && lands(at, c_jal_offset(u32::from(first))) {
                return true;
            }
            at += 2;
            continue;
        }
        let Some(instruction) = word(bytes, at) else {
            break;
        };
        let rd = (instruction >> 7) & 0x1F;
        if instruction & 0x7F == 0x6F && link(rd) && lands(at, jal_offset(instruction)) {
            return true;
        }
        // `auipc` and `jalr` through its register, the call the compiler gives a symbol before
        // the linker relaxes it.
        if instruction & 0x7F == 0x17
            && word(bytes, at + 4).is_some_and(|next| {
                next & 0x707F == 0x67
                    && link((next >> 7) & 0x1F)
                    && (next >> 15) & 0x1F == rd
                    && lands(
                        at,
                        sign_extend(instruction & 0xFFFF_F000, 32) + sign_extend(next >> 20, 12),
                    )
            })
        {
            return true;
        }
        at += 4;
    }
    false
}

/// The offset of RISC-V's `jal` `instruction`.
fn jal_offset(instruction: u32) -> i64 {
    let offset = (((instruction >> 31) & 1) << 20)
        | (((instruction >> 21) & 0x3FF) << 1)
        | (((instruction >> 20) & 1) << 11)
        | (((instruction >> 12) & 0xFF) << 12);
    sign_extend(offset, 21)
}

/// The offset of RISC-V's `c.jal` `instruction`.
fn c_jal_offset(instruction: u32) -> i64 {
    let offset = (((instruction >> 12) & 1) << 11)
        | (((instruction >> 11) & 1) << 4)
        | (((instruction >> 9) & 0b11) << 8)
        | (((instruction >> 8) & 1) << 10)
        | (((instruction >> 7) & 1) << 6)
        | (((instruction >> 6) & 1) << 7)
        | (((instruction >> 3) & 0b111) << 1)
        | (((instruction >> 2) & 1) << 5);
    sign_extend(offset, 12)
}

/// Whether the address `from`, counted from the start, and `offset` lead to the start.
fn lands(from: usize, offset: i64) -> bool {
    i64::try_from(from).is_ok_and(|from| from + offset == 0)
}

/// The little-endian 32-bit word of `bytes` at `at`.
fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let four = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(four.try_into().ok()?))
}

/// The little-endian halfword of `bytes` at `at`.
fn half(bytes: &[u8], at: usize) -> Option<u16> {
    let two = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes(two.try_into().ok()?))
}

/// The value of the `bits` lowest bits of `value`, read as a signed number.
fn sign_extend(value: u32, bits: u32) -> i64 {
    let shift = 64 - bits;
    (i64::from(value) << shift) >> shift
}

#[cfg(test)]
#[path = "self_calls_tests.rs"]
mod tests;
