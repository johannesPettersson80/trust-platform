//! The ESP32-C6's main stack gives a compiled program's entry points their task stack
//! (specification 34 MEM-03 and Appendix D.2; specification 44 §8.5).

use trust_platform_esp32::profile::{MAIN_STACK, NATIVE_TARGET};

/// The C1-25 bench read the stack's high-water over JTAG: 46,296 bytes for a program two call levels
/// of 6,144 bytes deep, so the deepest path of the firmware takes 46,296 − 2 × 6,144 = 34,008 bytes
/// before a program runs. The 78 KiB window, 79,872 bytes, less the 2 KiB headroom and that path,
/// leaves 79,872 − 2,048 − 34,008 = 43,816 bytes, the task stack the C6 offers a compiled program's
/// entry points. Wrong behaviour caught: figures that drift from the bench, a window smaller than the
/// firmware paints, and a task stack that does not keep the headroom.
#[test]
fn the_c6_main_stack_leaves_its_task_stack_to_compiled_programs() {
    assert_eq!(MAIN_STACK.window_bytes, 79_872);
    assert_eq!(MAIN_STACK.base_bytes, 46_296 - 2 * 6_144);
    assert_eq!(MAIN_STACK.task_stack_bytes(), 43_816);
    assert_eq!(NATIVE_TARGET.task_stack_bytes, 43_816);
}
