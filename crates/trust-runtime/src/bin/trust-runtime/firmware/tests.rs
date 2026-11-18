//! The tests of `trust-runtime firmware install` that need no chip: where each file of the firmware
//! folder is written, and which serial port is the chip's (specification 34 TARGET-04). The writes
//! themselves are proven on the chip (the C6 bench). Each expected address is the C6 chip file's.

use super::*;

/// A firmware folder with the three files, each beginning with its magic.
fn folder(name: &str) -> std::path::PathBuf {
    let folder = trust_test_support::unique_temp_path(&format!("trust-firmware-{name}"));
    std::fs::create_dir_all(&folder).expect("a folder");
    std::fs::write(folder.join("bootloader.bin"), [0xE9, 1, 2, 3]).expect("a file");
    std::fs::write(folder.join("partitions.bin"), [0xAA, 0x50, 1, 1]).expect("a file");
    std::fs::write(folder.join("firmware.bin"), [0xE9, 4, 5, 6]).expect("a file");
    folder
}

fn c6() -> ChipFile {
    Catalog::embedded()
        .expect("the chip files")
        .chip("esp32c6")
        .cloned()
        .expect("the C6's file")
}

/// The boot loader goes to 0x0, the partition table to 0x8000 and the firmware to 0x1_0000, the first
/// firmware slot (chips/esp32c6.toml); a file without its magic is refused. Catches a firmware written
/// into the second slot, which an erased selection never boots, and a folder of another chip or of a
/// half-copied release written to the flash.
#[test]
fn each_file_goes_where_the_chip_file_places_it() {
    let folder = folder("parts");
    let complete = parts(&c6(), &folder).expect("the folder is complete");
    let placed: Vec<(&str, u32)> = complete
        .iter()
        .map(|part| (part.name, part.address))
        .collect();
    assert_eq!(
        placed,
        vec![
            ("bootloader.bin", 0x0),
            ("partitions.bin", 0x8000),
            ("firmware.bin", 0x1_0000),
        ]
    );
    std::fs::write(folder.join("firmware.bin"), [0, 0, 0, 0]).expect("a file");
    let refused = parts(&c6(), &folder).expect_err("the image is refused");
    assert!(
        refused
            .to_string()
            .ends_with("firmware.bin is not a firmware.bin file of the chip"),
        "{refused}"
    );
    std::fs::remove_file(folder.join("partitions.bin")).expect("removed");
    assert!(parts(&c6(), &folder).is_err());
    let _ = std::fs::remove_dir_all(&folder);
}

fn usb(name: &str, vid: u16, pid: u16) -> SerialPortInfo {
    SerialPortInfo {
        port_name: name.into(),
        port_type: SerialPortType::UsbPort(UsbPortInfo {
            vid,
            pid,
            serial_number: None,
            manufacturer: None,
            product: None,
        }),
    }
}

/// The chip is the one serial port with the boot loader's USB identity, 303a:1001 for the C6; none
/// or two of them ask for `--port`. Catches a write to the first serial port found, which may be
/// another device.
#[test]
fn the_chip_is_the_one_port_with_its_boot_loaders_identity() {
    let ports = [
        usb("/dev/ttyUSB0", 0x0403, 0x6001),
        usb("/dev/ttyACM0", 0x303A, 0x1001),
    ];
    let (name, _) = find_port(&ports, 0x303A, 0x1001).expect("one chip");
    assert_eq!(name, "/dev/ttyACM0");
    assert_eq!(
        find_port(&ports[..1], 0x303A, 0x1001)
            .expect_err("no chip")
            .to_string(),
        "no chip on USB with the boot loader 303a:1001; give --port"
    );
    let two = [
        usb("/dev/ttyACM0", 0x303A, 0x1001),
        usb("/dev/ttyACM1", 0x303A, 0x1001),
    ];
    assert_eq!(
        find_port(&two, 0x303A, 0x1001)
            .expect_err("two chips")
            .to_string(),
        "two chips on USB with the boot loader 303a:1001, /dev/ttyACM0 and /dev/ttyACM1; give --port"
    );
}

/// The workspace lock that this build resolved. Included, so that a changed lock rebuilds the test.
const WORKSPACE_LOCK: &str = include_str!("../../../../../../Cargo.lock");

/// The version of every package `name` that the lock `lock` holds, as (major, minor, patch).
fn locked_versions(lock: &str, name: &str) -> Vec<(u64, u64, u64)> {
    let named = format!("name = \"{name}\"");
    let mut lines = lock.lines().map(str::trim);
    let mut versions = Vec::new();
    while let Some(line) = lines.next() {
        if line != named {
            continue;
        }
        let version = lines
            .next()
            .and_then(|line| line.strip_prefix("version = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("a locked package's version follows its name");
        let mut parts = version.split('.').map(|part| {
            part.parse().unwrap_or_else(|_| {
                panic!("{name} {version} is not a plain major.minor.patch version")
            })
        });
        let mut next = || parts.next().expect("major.minor.patch");
        versions.push((next(), next(), next()));
    }
    versions
}

// RUSTSEC-2026-0344 has no exception under specification 24, "Dependency Exceptions", so it must stay away: before 0.9.1,
// ruzstd decompressed a block without bounding its output, so a small crafted input exhausted memory. It came only
// through espflash 4.6.0 -> object 0.39, with which `firmware install` reads the ESP32 firmware it flashes; espflash
// 4.7.0's object 0.40 brings no ruzstd. Worked by hand from the advisory (patched: >= 0.9.1): the 0.8.3 that the lock
// held until 18 November 2026 is affected, and 0.9.1 is the first fixed version. Catches a dependency update or a merged
// lock file that brings an affected ruzstd back, and a reading of the lock that finds no package at all.
#[test]
fn no_locked_ruzstd_is_named_by_its_decompression_bomb_advisory() {
    let old_lock = "[[package]]\nname = \"ruzstd\"\nversion = \"0.8.3\"\n";
    assert_eq!(locked_versions(old_lock, "ruzstd"), [(0, 8, 3)]);
    assert!(
        !locked_versions(WORKSPACE_LOCK, "espflash").is_empty(),
        "the workspace lock holds espflash, which firmware install flashes with"
    );
    let affected: Vec<(u64, u64, u64)> = locked_versions(WORKSPACE_LOCK, "ruzstd")
        .into_iter()
        .filter(|version| *version < (0, 9, 1))
        .collect();
    assert!(
        affected.is_empty(),
        "ruzstd before 0.9.1 is named by RUSTSEC-2026-0344: {affected:?}"
    );
}
