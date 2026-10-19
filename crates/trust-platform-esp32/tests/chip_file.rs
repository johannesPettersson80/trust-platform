//! The ESP32-C6 data of this adapter is the chip's file, `chips/esp32c6.toml` (specification 34,
//! TARGET-03 and Appendix H): the chip ID, the clock, the partitions the firmware reads, and the
//! instruction set the firmware checks at boot. The firmware cannot read the file, so this host
//! test keeps the two equal. The `capabilities` reply the chip gives for its descriptor and its
//! last reset is checked against the file and the reset record as well (specification 11).

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_chips::{Catalog, ChipFile, Family, Fpu};
use trust_platform_esp32::profile::{
    numeric_profile_ok, CHIP_ID, CPU_HZ, DEVICE_DESCRIPTOR, FACTORY_OFFSET, FLASH_KIB,
    RESET_POWER_ON, RESET_USB, RESET_WATCHDOG, STBC_BYTES, STBC_OFFSET,
};
use trust_platform_esp32::program_flash::{SLOT_A_OFFSET, SLOT_BYTES, SLOT_B_OFFSET};
use trust_platform_esp32::{Platform, ResetCause, ResetRecord};
use trust_runtime_core::deployment::{ControlMode, DeviceDescriptor};
use trust_runtime_services::capabilities::{self, LastReset, ResetCauseName};
use trust_runtime_services::ports::StatusPort;
use trust_runtime_services::runner::EngineLiveValues;
use trust_runtime_services::status::StatusSettings;

fn chip_file() -> ChipFile {
    Catalog::embedded()
        .expect("the chip files are valid")
        .chip("esp32c6")
        .cloned()
        .expect("the ESP32-C6 has a chip file")
}

/// The `misa` an RV32 core with these extensions reads: MXL 1 in bits 31:30, one bit per letter.
fn misa(isa: &str) -> u32 {
    let extensions = isa.strip_prefix("RV32").expect("an RV32 instruction set");
    extensions
        .bytes()
        .fold(1 << 30, |misa, letter| misa | (1 << (letter - b'A')))
}

#[test]
fn the_c6_profile_has_the_identity_clock_and_partitions_of_the_chip_file() {
    let file = chip_file();
    assert_eq!(file.identity.model, "ESP32-C6");
    assert_eq!(file.identity.family, Family::Esp32);
    assert_eq!(file.identity.device_id, u32::from(CHIP_ID));
    // `CpuClock::max()`: the firmware runs the HP core at its highest clock.
    assert_eq!(file.clock.firmware_hz, CPU_HZ);
    assert_eq!(file.clock.max_hz, CPU_HZ);
    assert_eq!(file.layout.firmware.start, FACTORY_OFFSET);
    assert_eq!(file.layout.application.start, STBC_OFFSET);
    assert_eq!(
        file.layout.application.bytes(),
        u64::try_from(STBC_BYTES).expect("the application region is 6112 KiB")
    );
}

#[test]
fn the_instruction_set_of_the_chip_file_is_the_one_the_firmware_checks_at_boot() {
    let file = chip_file();
    assert!(numeric_profile_ok(misa(&file.processor.isa)));
    assert_eq!(file.processor.fpu, Fpu::None);
    // A chip with the F extension would fail the firmware's check.
    assert!(!numeric_profile_ok(misa("RV32IMAFC")));
}

#[test]
fn the_bench_module_flash_holds_the_whole_layout() {
    // FLASH_KIB is the 8 MB of the owned DevKitC-1's module, not a chip value: the C6's flash is
    // outside the chip, and its size is read when truST is installed.
    let file = chip_file();
    assert!(file.memory.flash.is_none());
    let end = file
        .layout
        .regions()
        .into_iter()
        .map(|(_, region)| region.end())
        .max()
        .expect("a layout");
    assert!(end <= u64::from(FLASH_KIB) * 1024);
}

/// The device descriptor admission reads is the chip file's program slot and its `[events]` table;
/// the maxima for the preparation limits are trust-runtime's hosted maxima because
/// `chips/esp32c6.toml` names none yet (specification 34 §5.1.1: "Appendix H gains the keys for
/// its maxima with the chip's firmware"). Catches a slot that is not the chip's program slot (for
/// example the 16 KiB bench bundle), an event store that is not the chip file's, and maxima that
/// were invented instead of taken from the chip file or from trust-runtime.
#[test]
fn the_device_descriptor_matches_the_chip_file() {
    let file = chip_file();
    assert_eq!(
        DEVICE_DESCRIPTOR.application_slot_bytes,
        Some(
            file.layout
                .application_a
                .expect("the ESP32-C6 names its program slots")
                .bytes()
        )
    );
    assert_eq!(
        DEVICE_DESCRIPTOR.event_store_default,
        file.events.default_capacity
    );
    assert_eq!(DEVICE_DESCRIPTOR.event_store_max, file.events.max_capacity);
    // No maxima keys yet: the descriptor carries trust-runtime's hosted maxima, which the
    // chip file will replace when Appendix H gains them.
    assert_eq!(DEVICE_DESCRIPTOR.maxima, DeviceDescriptor::hosted().maxima);
}

/// The device offers one program slot, not the whole application region (specification 34 §5.1 and
/// §5.1.1 step 8; Appendix H.2): an install writes one of the two slots `application_a` and
/// `application_b` of the chip file, each `kib = 3056`, 3,056 x 1,024 = 3,129,344 bytes, and the two
/// together are the 6,112 KiB `application` region. A program and its record that fit the region but
/// not a slot could never be installed. Catches a descriptor that still offers the region
/// (6,258,688 bytes), which admission would grant to a container that no slot can take.
#[test]
fn the_device_descriptor_offers_one_slot() {
    let file = chip_file();
    let slot_a = file
        .layout
        .application_a
        .expect("the ESP32-C6 names slot A");
    let slot_b = file
        .layout
        .application_b
        .expect("the ESP32-C6 names slot B");
    assert_eq!(slot_a.bytes(), 3_129_344);
    assert_eq!(slot_b.bytes(), 3_129_344);
    assert_eq!(
        slot_a.bytes() + slot_b.bytes(),
        file.layout.application.bytes(),
        "the two slots are the application region"
    );
    assert_eq!(DEVICE_DESCRIPTOR.application_slot_bytes, Some(3_129_344));
    assert_ne!(
        DEVICE_DESCRIPTOR.application_slot_bytes,
        Some(file.layout.application.bytes()),
        "the descriptor does not offer the whole region"
    );
    assert_eq!(
        DEVICE_DESCRIPTOR.application_slot_bytes,
        Some(u64::from(SLOT_BYTES)),
        "and it is the slot the program store writes"
    );
}

/// The chip file names both program slots of the program store, and the offsets and size that
/// `program_flash.rs` drives are the file's (specification 34 §5.1, Appendix D.2 and Appendix H.2;
/// decision G14). Worked out by hand from D.2: the `stbc` region of 6,112 KiB at 0x20_0000 is two
/// slots of 3,056 KiB, so slot A is 3,056 x 1,024 = 3,129,344 bytes at 0x20_0000, slot B follows it at
/// 0x20_0000 + 0x2F_C000 = 0x4F_C000 and ends at 0x7F_8000, where the first checkpoint slot begins.
/// Catches a chip file whose slots differ from the constants the store writes with, which would
/// send an install into the firmware or a checkpoint slot, a slot B that leaves a gap after slot A or
/// overlaps it, and slots that do not fill the `application` region.
#[test]
fn the_chip_file_names_both_application_slots() {
    let file = chip_file();
    let slot_a = file
        .layout
        .application_a
        .expect("the ESP32-C6 names slot A");
    let slot_b = file
        .layout
        .application_b
        .expect("the ESP32-C6 names slot B");

    // The values of the specification, as literals.
    assert_eq!((slot_a.start, slot_a.kib), (0x20_0000, 3_056));
    assert_eq!((slot_b.start, slot_b.kib), (0x4F_C000, 3_056));

    // `program_flash.rs` drives exactly the file's slots.
    assert_eq!(slot_a.start, SLOT_A_OFFSET);
    assert_eq!(slot_b.start, SLOT_B_OFFSET);
    assert_eq!(slot_a.bytes(), u64::from(SLOT_BYTES));
    assert_eq!(slot_b.bytes(), u64::from(SLOT_BYTES));

    // The slots follow each other and fill the `application` region, up to the checkpoint slots.
    assert_eq!(slot_a.end(), u64::from(slot_b.start));
    assert_eq!(file.layout.application.start, slot_a.start);
    assert_eq!(slot_b.end(), file.layout.application.end());
    assert_eq!(slot_b.end(), u64::from(file.layout.checkpoint_a.start));
}

/// The `capabilities` reply of the chip's descriptor carries the chip file's program slot and
/// event store (specification 34 §5.1.1 and TARGET-03; specification 11, the runtime device
/// capabilities contract). The slot is `kib = 3056` of `[layout] application_a`, 3056 x 1024 bytes;
/// the event store is the file's `[events]` table. The reply is rendered from the descriptor by the
/// shared services, so the rendered fields are compared with the file and with literals worked out
/// by hand here. Catches a slot in another unit or region (the 16 KiB bench bundle, or the whole
/// 6,112 KiB region), an event store that is the computer's 1,000, and an unbounded maximum written
/// as a number instead of `null`.
#[test]
fn the_capabilities_reply_of_the_chip_carries_the_chip_file_slot_and_event_store() {
    let file = chip_file();
    let reply = capabilities::descriptor_json(&DEVICE_DESCRIPTOR, None);
    assert_eq!(reply["application_slot_bytes"], json!(3_129_344_u64));
    assert_eq!(
        reply["application_slot_bytes"],
        json!(file
            .layout
            .application_a
            .expect("the ESP32-C6 names its program slots")
            .bytes())
    );
    assert_eq!(
        reply["event_store_default"],
        json!(file.events.default_capacity)
    );
    assert_eq!(reply["event_store_max"], json!(file.events.max_capacity));
    assert_eq!(reply["timing_evidence"], json!("unqualified"));
    // The C6 has no wall clock until slice T6 (profile.rs), so no capability is reported.
    assert_eq!(reply["capabilities"], json!([]));
    // Specification 34 §5.1.1: a 64 MiB artifact and 1,024 call levels are bounded; the other
    // limits are `u64::MAX` in the descriptor, no maximum, and render as `null`.
    assert_eq!(reply["maxima"]["max_artifact_bytes"], json!(67_108_864));
    assert_eq!(reply["maxima"]["max_call_depth"], json!(1_024));
    assert_eq!(reply["maxima"]["max_preparation_bytes"], Value::Null);
}

/// A platform over no watchdog that reports the reset `last_reset` its boot reported.
fn platform_with(last_reset: LastReset) -> Platform<()> {
    Platform::new(
        (),
        SmolStr::new_static("Controller"),
        StatusSettings {
            execution_backend: "vm",
            execution_backend_source: "default",
            simulation_enabled: false,
            simulation_time_scale: 1,
            simulation_mode: SmolStr::new_static("production"),
            simulation_warning: SmolStr::default(),
        },
        false,
        ControlMode::Production,
        EngineLiveValues::empty(),
        last_reset,
        trust_platform_esp32::profile::runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC0]),
    )
}

/// Specification 11, `status`: a chip's runtime ID is its chip name and USB serial number, and
/// it reports its chip as its platform and keeps control after `shutdown` (spec 34 STATE-08).
/// Catches a runtime ID that is the same for two chips of one model (the chip name alone), a
/// MAC in another case or order than the USB serial shows, and a chip reporting `process`.
#[test]
fn the_c6_reports_its_chip_its_usb_serial_and_that_it_keeps_control() {
    let platform = platform_with(Platform::<()>::boot_last_reset(1, None, 1));
    assert_eq!(StatusPort::platform(&platform), "esp32c6");
    assert_eq!(
        StatusPort::runtime_id(&platform),
        "esp32c6-20:6E:F1:17:04:C0"
    );
    assert_eq!(
        StatusPort::program_control(&platform),
        trust_runtime_services::ports::ProgramControl::KeepsControl
    );
    let other = trust_platform_esp32::profile::runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC1]);
    assert_ne!(
        other,
        StatusPort::runtime_id(&platform),
        "two chips have two runtime IDs"
    );
}

/// The reset the chip reports for a boot: its cause, its text and the boot counter (specification 11,
/// the runtime device capabilities contract).
fn reset_of(cause: ResetCauseName, text: &str, boot_count: u32) -> LastReset {
    LastReset {
        cause,
        text: SmolStr::new(text),
        boot_count,
    }
}

/// The chip reports the reset its boot read (specification 11, the runtime device capabilities
/// contract; specification 34, the fixed-size reset record). A record the previous boot left gives
/// its cause and text, and the boot counter is the chip's, for this boot: the record's own counter
/// (3 below) is not reported. A boot with no record takes its cause from the hardware reset reason:
/// a watchdog reset (`RESET_WATCHDOG`, 0x07) writes no record and reports `watchdog`, while a
/// power-on or host (USB) reset reports `power_on`. A record wins over the reason. Catches a record
/// the firmware hands over that the platform drops, a cause renamed or mapped to another, a text
/// lost, a boot counter taken from the record or left at 0, a watchdog reset reported as `power_on`
/// (the reply the bench checks), and a reason that overrides a record.
#[test]
fn the_chip_reports_the_reset_its_boot_read() {
    // A record the previous boot left, stamped with boot 3, is reported with this boot's counter 7.
    for (cause, name) in [
        (ResetCause::Watchdog, ResetCauseName::Watchdog),
        (ResetCause::Panic, ResetCauseName::Panic),
        (ResetCause::Requested, ResetCauseName::Requested),
    ] {
        let record = ResetRecord::with_text(cause, "scan", 3);
        let boot = Platform::<()>::boot_last_reset(RESET_POWER_ON, Some(record), 7);
        assert_eq!(
            boot,
            reset_of(name, "scan", 7),
            "the record's {cause:?} reads as {} with this boot's counter",
            name.as_str()
        );
        assert_eq!(
            platform_with(boot).last_reset(),
            Some(reset_of(name, "scan", 7)),
            "the platform reports the reset its boot reported"
        );
    }
    // A watchdog reset writes no record: its reason, 0x07, reads as `watchdog` with empty text.
    let watchdog = Platform::<()>::boot_last_reset(RESET_WATCHDOG, None, 4);
    assert_eq!(
        watchdog,
        reset_of(ResetCauseName::Watchdog, "", 4),
        "a hardware watchdog reset with no record reads as watchdog"
    );
    // A power-on or host (USB) reset with no record reads as `power_on`, with this boot's counter.
    for (reason, count) in [(RESET_POWER_ON, 1), (RESET_USB, 2)] {
        let boot = Platform::<()>::boot_last_reset(reason, None, count);
        assert_eq!(
            boot,
            reset_of(ResetCauseName::PowerOn, "", count),
            "reason {reason:#04x} with no record reads as power_on"
        );
    }
    // A record the boot read wins over the watchdog reason.
    let record = ResetRecord::with_text(ResetCause::Panic, "main.rs:42", 5);
    let record_wins = Platform::<()>::boot_last_reset(RESET_WATCHDOG, Some(record), 6);
    assert_eq!(
        record_wins,
        reset_of(ResetCauseName::Panic, "main.rs:42", 6),
        "a record the boot read wins over the watchdog reason"
    );
}
