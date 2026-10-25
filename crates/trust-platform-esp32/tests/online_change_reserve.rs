//! The heap that the C6 keeps free beside the running program (specification 34 §11.2, §11.3,
//! MEM-12, UPDATE-06 and Appendix D.2): the working memory of an online change,
//! `ONLINE_CHANGE_WORKING_MEMORY_BYTES` (8,192 bytes), and the checkpoint store's payload buffer,
//! which the memory the C6 reports for programs leaves out.

use trust_platform_esp32::loader::program_memory_bytes;

/// The largest payload a checkpoint slot of the C6 takes: a 16 KiB slot of 4 KiB erase units and
/// 4-byte program units, less the slot header, a record header and the commit field (specification
/// 34 §10.1).
const LARGEST_PAYLOAD: usize = 16_300;

#[test]
fn the_c6_has_for_programs_its_free_heap_less_what_it_keeps_beside_them() {
    // Specification 34 MEM-12 and Appendix D.2: the C6 builds its resource loop before it reads any
    // container, and its memory for programs is the heap's free bytes then, less the working memory of
    // an online change (8,192 bytes) and the largest payload that a checkpoint slot takes (16,300
    // bytes: the checkpoint store's buffer, which a warm start also reads the newest checkpoint into).
    // Worked by hand with 120,000 bytes free: 120,000 - 8,192 - 16,300 = 95,508 bytes. The wrong
    // behaviour: the working memory or the checkpoint store left out (a program admitted that the
    // chip could not then run), or the loop, already taken from the free bytes, counted again (a
    // program that fits refused).
    assert_eq!(program_memory_bytes(120_000, LARGEST_PAYLOAD), 95_508);
    // A heap that holds less than the chip keeps beside programs leaves no memory for programs.
    assert_eq!(program_memory_bytes(10_000, LARGEST_PAYLOAD), 0);
}
