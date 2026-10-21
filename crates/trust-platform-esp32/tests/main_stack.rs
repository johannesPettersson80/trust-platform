//! The ESP32-C6's main stack decides its call levels (specification 34 MEM-03 and Appendix D.2;
//! owner, 21 October 2026, "Stack decides").

use trust_platform_esp32::profile::MAIN_STACK;

/// The C1-25 bench installed programs whose chains of nested function blocks need 2, 3, 4 and 6
/// call levels and read the stack's high-water over JTAG: 46,296, 52,440, 58,584 and 70,872 bytes,
/// so each level takes 6,144 bytes and the path 46,296 − 2 × 6,144 = 34,008 before its first. The
/// 78 KiB window less the 2 KiB headroom holds (79,872 − 2,048 − 34,008) / 6,144 = 7 levels, and the
/// deepest admitted install keeps the headroom: 34,008 + 7 × 6,144 + 2,048 = 79,064 bytes, within
/// 79,872. Wrong behaviour caught: figures that drift from the bench, a window smaller than the
/// firmware paints, and a call depth that the window does not hold.
#[test]
fn the_c6_main_stack_holds_seven_call_levels() {
    assert_eq!(MAIN_STACK.window_bytes, 79_872);
    for (levels, peak) in [(2, 46_296), (3, 52_440), (4, 58_584), (6, 70_872)] {
        assert_eq!(
            MAIN_STACK.base_bytes + levels * MAIN_STACK.level_bytes,
            peak,
            "{levels} call levels"
        );
    }
    assert_eq!(MAIN_STACK.call_depth(), 7);
    assert_eq!(
        MAIN_STACK.base_bytes + 7 * MAIN_STACK.level_bytes + 2_048,
        79_064
    );
}
