//! The C6 image's RAM code check (checklist G217): a function's own module, and the runner's control
//! path in RAM named while the scan path is not.

use super::*;

/// `LentProgram::run` of the C6 firmware of 18 November 2026: a method of the runner's stepped
/// scans whose generic arguments name the C6's GPIO type, which the GPIO pattern matched.
const STEPPED_RUN: &str = "_RNvMs0_NtNtCs70wmMUVa4Zz_22trust_runtime_services6runner8steppingINtB5_11LentProgramINtNtB7_8compiled13CompiledCycleNtCs2Mdu476PG2f_13trust_esp32c69ChipClockINtNtNtB9_4chip10process_io12ProcessImageNtNtNtCs5z7qdphMeHS_20trust_platform_esp3210process_io4gpio9Esp32PinsENtNtNtB3g_8hardware11code_region6C6CodeEB1Q_E3runB1S_";
/// A method of the loop's scan step, whose arguments name the stepped scans' module: the scan
/// path, in RAM.
const SUPERVISION: &str = "_RNvMNtNtCs70wmMUVa4Zz_22trust_runtime_services6runner11supervisionINtB4_6RunnerINtNtB6_8stepping11LentProgramE4scan";
/// The GPIO read a scan makes, in RAM by `#[ram]`.
const GPIO_READ: &str = "_RNvXs_NtNtCs5z7qdphMeHS_20trust_platform_esp3210process_io4gpioNtB4_9Esp32PinsNtNtCs70wmMUVa4Zz_22trust_runtime_services4chip8GpioPins7is_high";

#[test]
fn a_symbols_own_module_is_the_one_it_names_first() {
    // Catches a reading that takes a generic argument's module for the function's own: the
    // stepped scan names the GPIO module in its arguments, the scan step the stepped scans'.
    assert_eq!(
        own_module(STEPPED_RUN).as_deref(),
        Some("trust_runtime_services::runner::stepping")
    );
    assert_eq!(
        own_module(SUPERVISION).as_deref(),
        Some("trust_runtime_services::runner::supervision")
    );
    assert_eq!(
        own_module(GPIO_READ).as_deref(),
        Some("trust_platform_esp32::process_io::gpio")
    );
    assert_eq!(own_module("_ZN4core3ptr13drop_in_place17h0123456789abcdefE"), None);
    assert_eq!(own_module("memcpy"), None);
}

#[test]
fn the_control_path_in_ram_is_named_and_the_scan_path_is_not() {
    // The stepped scan in RAM is refused by its own module; the scan step that takes its type
    // as an argument, and the GPIO read, are the scan path; the stepped scan in flash is where
    // it belongs. Catches a check that reads every path of a symbol, which would refuse the
    // scan step, and one that misses the stepped scan.
    let ram = 0x4080_0000..0x4088_0000;
    let symbols = [
        (STEPPED_RUN, 0x4081_0000),
        (SUPERVISION, 0x4081_2000),
        (GPIO_READ, 0x4081_3000),
        (STEPPED_RUN, 0x4200_0000),
    ];
    assert_eq!(control_path_in_ram(symbols, &ram), [STEPPED_RUN.to_string()]);
}
