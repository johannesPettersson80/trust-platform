//! A function's direct call of itself is found from its call instruction (specification 48
//! RU-18); each code below is assembled by hand from the target's instruction encodings.

use trust_runtime_core::native::image::Target;

use super::{calls_itself, x86_64_instruction};

#[test]
fn a_call_of_the_functions_own_start_is_found_on_every_target() {
    // RU-18: `rules::fib` called itself on x86-64 without a call relocation, and the call graph
    // missed it. Catches a call of the start missed on a target, and a loop's jump back to the
    // start or a call of another place taken for one.
    let cases: [(Target, &[u8], bool); 16] = [
        // push rbp; call -6 (the start); ret
        (
            Target::X86_64,
            &[0x55, 0xE8, 0xFA, 0xFF, 0xFF, 0xFF, 0xC3],
            true,
        ),
        // push rbp; jmp -6 (a loop); ret
        (
            Target::X86_64,
            &[0x55, 0xE9, 0xFA, 0xFF, 0xFF, 0xFF, 0xC3],
            false,
        ),
        // push rbp; call +16 (another function); ret
        (
            Target::X86_64,
            &[0x55, 0xE8, 0x10, 0x00, 0x00, 0x00, 0xC3],
            false,
        ),
        // nop; nop; bl -8 (the start); ret
        (
            Target::Aarch64,
            &[
                0x1F, 0x20, 0x03, 0xD5, 0x1F, 0x20, 0x03, 0xD5, 0xFE, 0xFF, 0xFF, 0x97, 0xC0, 0x03,
                0x5F, 0xD6,
            ],
            true,
        ),
        // nop; nop; b -8 (a loop); ret
        (
            Target::Aarch64,
            &[
                0x1F, 0x20, 0x03, 0xD5, 0x1F, 0x20, 0x03, 0xD5, 0xFE, 0xFF, 0xFF, 0x17, 0xC0, 0x03,
                0x5F, 0xD6,
            ],
            false,
        ),
        // nop; nop; bl -8 (the start: its address + 4 - 8); bx lr
        (
            Target::ThumbV8mMainHf,
            &[0x00, 0xBF, 0x00, 0xBF, 0xFF, 0xF7, 0xFC, 0xFF, 0x70, 0x47],
            true,
        ),
        // nop; nop; b.w -8 (a loop); bx lr
        (
            Target::ThumbV8mMainHf,
            &[0x00, 0xBF, 0x00, 0xBF, 0xFF, 0xF7, 0xFC, 0xBF, 0x70, 0x47],
            false,
        ),
        // nop; jal ra, -4 (the start); ret
        (
            Target::Rv32imac,
            &[
                0x13, 0x00, 0x00, 0x00, 0xEF, 0xF0, 0xDF, 0xFF, 0x67, 0x80, 0x00, 0x00,
            ],
            true,
        ),
        // nop; j -4 (a loop); ret
        (
            Target::Rv32imac,
            &[
                0x13, 0x00, 0x00, 0x00, 0x6F, 0xF0, 0xDF, 0xFF, 0x67, 0x80, 0x00, 0x00,
            ],
            false,
        ),
        // c.nop; c.jal -2 (the start); c.ret
        (
            Target::Rv32imac,
            &[0x01, 0x00, 0xFD, 0x3F, 0x82, 0x80],
            true,
        ),
        // c.nop; c.j -2 (a loop); c.ret
        (
            Target::Rv32imac,
            &[0x01, 0x00, 0xFD, 0xBF, 0x82, 0x80],
            false,
        ),
        // nop; auipc ra, 0; jalr ra, -4(ra) (the start: 4 + 0 - 4); ret
        (
            Target::Rv32imac,
            &[
                0x13, 0x00, 0x00, 0x00, 0x97, 0x00, 0x00, 0x00, 0xE7, 0x80, 0xC0, 0xFF, 0x67, 0x80,
                0x00, 0x00,
            ],
            true,
        ),
        // auipc ra, 0; jalr ra, 8(ra) (another place); ret
        (
            Target::Rv32imac,
            &[
                0x97, 0x00, 0x00, 0x00, 0xE7, 0x80, 0x80, 0x00, 0x67, 0x80, 0x00, 0x00,
            ],
            false,
        ),
        // nop; auipc t1, 0; jalr zero, -4(t1) (a jump back to the start, without a link)
        (
            Target::Rv32imac,
            &[
                0x13, 0x00, 0x00, 0x00, 0x17, 0x03, 0x00, 0x00, 0x67, 0x00, 0xC3, 0xFF,
            ],
            false,
        ),
        // An empty function and a call cut short call nothing.
        (Target::X86_64, &[], false),
        (Target::X86_64, &[0xE8], false),
    ];
    for (target, code, expected) in cases {
        assert_eq!(
            calls_itself(code, target),
            expected,
            "{target:?} {code:02X?}"
        );
    }
}

#[test]
fn a_byte_inside_an_instruction_is_never_taken_for_a_call() {
    // RU-18: a function that does not call itself is not refused. Catches a reading of every byte
    // or halfword as a possible call, which takes a constant or an instruction's upper half that
    // looks like a call of the start for one.
    let cases: [(Target, &[u8], bool); 5] = [
        // mov dword [rbp-0x18], -7; ret: read from byte 2, `E8 F9 FF FF FF` is a call of -7 from
        // byte 7, the start.
        (
            Target::X86_64,
            &[0xC7, 0x45, 0xE8, 0xF9, 0xFF, 0xFF, 0xFF, 0xC3],
            false,
        ),
        // endbr64; push rbp; mov rbp, rsp; mov dword [rbp-0x18], -15 (`E8 F1 FF FF FF` from byte
        // 10 would lead to the start); lea rax, [rip-0x18]; movdqa xmm0, xmm1;
        // vpermq ymm0, ymm1, 0x1B; vzeroupper; nop word cs:[rax+rax]; call -50 (at 45, ending at
        // 50: the start); pop rbp; ret
        (Target::X86_64, &LONG_X86_64, true),
        // The same with the call's displacement one short: no call of the start.
        (Target::X86_64, &long_x86_64_calling(-51), false),
        // An instruction the decoder does not know (`push es`, which 64-bit mode lacks), then
        // call -6 (the start): from there on every byte is read, and the call is found.
        (Target::X86_64, &[0x06, 0xE8, 0xFA, 0xFF, 0xFF, 0xFF], true),
        // addi zero, s10, 1023 (its upper half 0x3FFD is `c.jal -2`, which would lead from byte 2
        // to the start); c.ret
        (
            Target::Rv32imac,
            &[0x13, 0x00, 0xFD, 0x3F, 0x82, 0x80],
            false,
        ),
    ];
    for (target, code, expected) in cases {
        assert_eq!(
            calls_itself(code, target),
            expected,
            "{target:?} {code:02X?}"
        );
    }
}

/// The function of `a_byte_inside_an_instruction_is_never_taken_for_a_call` that calls itself.
const LONG_X86_64: [u8; 52] = long_x86_64_calling(-50);

/// That function with the call's displacement `displacement`.
const fn long_x86_64_calling(displacement: i32) -> [u8; 52] {
    let call = displacement.to_le_bytes();
    [
        0xF3, 0x0F, 0x1E, 0xFA, // endbr64
        0x55, // push rbp
        0x48, 0x89, 0xE5, // mov rbp, rsp
        0xC7, 0x45, 0xE8, 0xF1, 0xFF, 0xFF, 0xFF, // mov dword [rbp-0x18], -15
        0x48, 0x8D, 0x05, 0xE8, 0xFF, 0xFF, 0xFF, // lea rax, [rip-0x18]
        0x66, 0x0F, 0x6F, 0xC1, // movdqa xmm0, xmm1
        0xC4, 0xE3, 0xFD, 0x00, 0xC1, 0x1B, // vpermq ymm0, ymm1, 0x1B
        0xC5, 0xF8, 0x77, // vzeroupper
        0x66, 0x2E, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, // nop word cs:[rax+rax]
        0xE8, call[0], call[1], call[2], call[3], // call
        0x5D,    // pop rbp
        0xC3,    // ret
    ]
}

#[test]
fn every_x86_64_instruction_has_its_encodings_length() {
    // RU-18: x86-64 code is read one instruction after the other, so every length must be the
    // encoding's. Catches a prefix, REX, ModRM, SIB, displacement or immediate counted wrong, and
    // a two-byte, three-byte, VEX, EVEX or XOP form taken for another.
    let cases: [(&[u8], usize, Option<i64>); 46] = [
        (&[0x55], 1, None),                                     // push rbp
        (&[0xCC], 1, None),                                     // int3
        (&[0x48, 0x89, 0xE5], 3, None),                         // mov rbp, rsp
        (&[0x41, 0x57], 2, None),                               // push r15
        (&[0x48, 0x83, 0xEC, 0x28], 4, None),                   // sub rsp, 0x28
        (&[0x48, 0x81, 0xEC, 0x00, 0x01, 0x00, 0x00], 7, None), // sub rsp, 0x100
        (&[0x66, 0x81, 0xC1, 0x34, 0x12], 5, None),             // add cx, 0x1234
        (&[0xB8, 0x01, 0x00, 0x00, 0x00], 5, None),             // mov eax, 1
        (&[0x66, 0xB8, 0x01, 0x00], 4, None),                   // mov ax, 1
        (
            &[0x48, 0xB8, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
            10,
            None,
        ), // movabs rax
        (&[0xC7, 0x45, 0xE8, 0xF9, 0xFF, 0xFF, 0xFF], 7, None), // mov dword [rbp-0x18], -7
        (&[0x48, 0x8D, 0x05, 0xE8, 0xFF, 0xFF, 0xFF], 7, None), // lea rax, [rip-0x18]
        (&[0x89, 0x44, 0x24, 0x10], 4, None),                   // mov [rsp+0x10], eax
        (&[0x8B, 0x04, 0x25, 0x78, 0x56, 0x34, 0x12], 7, None), // mov eax, [0x12345678]
        (&[0x8B, 0x84, 0x8B, 0x00, 0x01, 0x00, 0x00], 7, None), // mov eax, [rbx+rcx*4+0x100]
        (&[0xF6, 0x07, 0x01], 3, None),                         // test byte [rdi], 1
        (&[0xF7, 0xC0, 0x00, 0x00, 0x01, 0x00], 6, None),       // test eax, 0x10000
        (&[0xF7, 0xD8], 2, None),                               // neg eax
        (&[0x6B, 0xC0, 0x0A], 3, None),                         // imul eax, eax, 10
        (&[0x69, 0xC0, 0x10, 0x27, 0x00, 0x00], 6, None),       // imul eax, eax, 10000
        (&[0x74, 0x05], 2, None),                               // je +5
        (&[0x0F, 0x84, 0x00, 0x01, 0x00, 0x00], 6, None),       // je +0x100
        (&[0xE9, 0x00, 0x00, 0x00, 0x00], 5, None),             // jmp +0
        (&[0xE8, 0x10, 0x00, 0x00, 0x00], 5, Some(16)),         // call +16
        (&[0xE8, 0xFB, 0xFF, 0xFF, 0xFF], 5, Some(-5)),         // call -5
        (&[0xC2, 0x08, 0x00], 3, None),                         // ret 8
        (&[0xC8, 0x10, 0x00, 0x00], 4, None),                   // enter 0x10, 0
        (
            &[0xA1, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
            9,
            None,
        ), // mov eax, [moffs64]
        (&[0x67, 0xA1, 0x44, 0x33, 0x22, 0x11], 6, None),       // mov eax, [moffs32]
        (&[0xD9, 0x45, 0xF8], 3, None),                         // fld dword [rbp-8]
        (&[0x0F, 0x0B], 2, None),                               // ud2
        (&[0x0F, 0xAE, 0xE8], 3, None),                         // lfence
        (&[0x0F, 0xBA, 0xE0, 0x03], 4, None),                   // bt eax, 3
        (&[0xF0, 0x48, 0x0F, 0xB1, 0x0F], 5, None),             // lock cmpxchg [rdi], rcx
        (&[0xF3, 0x0F, 0xB8, 0xC1], 4, None),                   // popcnt eax, ecx
        (&[0x0F, 0x1F, 0x44, 0x00, 0x00], 5, None),             // nop dword [rax+rax]
        (
            &[0x66, 0x2E, 0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
            10,
            None,
        ), // 10-byte nop
        (&[0x66, 0x0F, 0x6F, 0xC1], 4, None),                   // movdqa xmm0, xmm1
        (&[0x66, 0x0F, 0x38, 0x00, 0xC1], 5, None),             // pshufb xmm0, xmm1
        (&[0x66, 0x0F, 0x3A, 0x0F, 0xC1, 0x08], 6, None),       // palignr xmm0, xmm1, 8
        (&[0x0F, 0x0F, 0xC1, 0xB4], 4, None),                   // pfmul mm0, mm1 (3DNow!)
        (&[0xC5, 0xF8, 0x77], 3, None),                         // vzeroupper
        (&[0xC5, 0xFC, 0x10, 0x07], 4, None),                   // vmovups ymm0, [rdi]
        (&[0xC4, 0xE3, 0xFD, 0x00, 0xC1, 0x1B], 6, None),       // vpermq ymm0, ymm1, 0x1B
        (&[0x62, 0xF1, 0x7C, 0x48, 0x10, 0x07], 6, None),       // vmovups zmm0, [rdi]
        (&[0x62, 0xF3, 0xFD, 0x48, 0x00, 0xC1, 0x1B], 7, None), // vpermq zmm0, zmm1, 0x1B
    ];
    for (code, length, call) in cases {
        assert_eq!(
            x86_64_instruction(code),
            Some((length, call)),
            "{code:02X?}"
        );
    }
    // `pop rax` through ModRM, whose register field 0 keeps it from XOP; XOP's vprotb with an
    // immediate (map 8).
    assert_eq!(x86_64_instruction(&[0x8F, 0xC0]), Some((2, None)));
    assert_eq!(
        x86_64_instruction(&[0x8F, 0xE8, 0x78, 0xC0, 0xC1, 0x05]),
        Some((6, None))
    );
    // VIA PadLock's rep xcrypt-ecb, whose third byte is a ModRM byte.
    assert_eq!(
        x86_64_instruction(&[0xF3, 0x0F, 0xA7, 0xC8]),
        Some((4, None))
    );
    // An instruction cut short, and one 64-bit mode lacks.
    assert_eq!(x86_64_instruction(&[0xE8, 0x00, 0x00]), None);
    assert_eq!(x86_64_instruction(&[0x06]), None);
}
