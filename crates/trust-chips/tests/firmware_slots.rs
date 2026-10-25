//! The firmware slots, the boot loader's selection, the device settings sectors and the pairing
//! sectors in a chip file (specification 34 TARGET-04, TARGET-02 and Appendix H.2; specification 11
//! §6.9.6). Each test edits a real chip file and
//! checks that the edit is refused with the problem of the broken rule, or that an edit within the
//! rules is accepted.

use trust_chips::{ChipFile, Problem};

const C6: &str = include_str!("../../../chips/esp32c6.toml");

/// A TOML text as a table.
fn table(text: &str) -> toml::Table {
    text.parse().expect("TOML")
}

/// The table at a dotted path of the file.
fn at<'a>(file: &'a mut toml::Table, path: &str) -> &'a mut toml::Table {
    path.split('.').fold(file, |table, key| {
        table
            .get_mut(key)
            .and_then(toml::Value::as_table_mut)
            .expect("a table of the file")
    })
}

/// Sets `key` in the table at `path`.
fn set(file: &mut toml::Table, path: &str, key: &str, value: impl Into<toml::Value>) {
    at(file, path).insert(key.to_owned(), value.into());
}

/// Removes `key` from the table at `path`.
fn remove(file: &mut toml::Table, path: &str, key: &str) {
    at(file, path).remove(key).expect("the key is there");
}

/// A chip file after `edit`, read and checked.
fn edited(
    name: &str,
    text: &str,
    edit: impl FnOnce(&mut toml::Table),
) -> Result<ChipFile, Problem> {
    let mut file = table(text);
    edit(&mut file);
    let text = toml::to_string(&file).expect("a table writes as TOML");
    ChipFile::parse(name, &text).map_err(|error| error.problem)
}

fn c6(edit: impl FnOnce(&mut toml::Table)) -> Result<ChipFile, Problem> {
    edited("esp32c6", C6, edit)
}

/// The second firmware slot and the selection are named both or neither, and the second slot is the
/// first's size: the C6's real file (1,984 KiB at `0x1_0000` and at `0x20_0000`, the selection at
/// `0xB000`)
/// is accepted. Catches a file whose update has no selection to switch, or no slot to write, and a
/// second slot smaller than the first, which an update of a firmware that fits the first would
/// overrun.
#[test]
fn the_firmware_slots_are_named_with_their_selection_and_of_one_size() {
    assert_eq!(c6(|_| {}).err(), None);
    assert_eq!(
        c6(|file| remove(file, "layout", "firmware_select")),
        Err(Problem::MissingRegion {
            region: "firmware_select"
        })
    );
    assert_eq!(
        c6(|file| remove(file, "layout", "firmware_b")),
        Err(Problem::MissingRegion {
            region: "firmware_b"
        })
    );
    let neither = c6(|file| {
        remove(file, "layout", "firmware_b");
        remove(file, "layout", "firmware_select");
    });
    assert_eq!(neither.err(), None);
    assert_eq!(
        c6(|file| set(file, "layout.firmware_b", "kib", 1_920_i64)),
        Err(Problem::UnequalSlots {
            first: "firmware",
            second: "firmware_b"
        })
    );
}

/// The device settings sectors are named both or neither, each exactly one erase unit: a write of the
/// settings erases one whole sector. 8 KiB from 0xD000 runs into `device_b` at 0xE000, and is refused
/// as two units once `device_b` moves out of its way to 0xF000. Catches a settings write that would
/// erase a neighbouring region.
#[test]
fn the_device_settings_are_named_in_pairs_of_one_erase_unit() {
    assert_eq!(
        c6(|file| remove(file, "layout", "device_b")),
        Err(Problem::MissingRegion { region: "device_b" })
    );
    assert_eq!(
        c6(|file| {
            set(file, "layout.device_b", "start", 0xF000_i64);
            set(file, "layout.device_a", "kib", 8_i64);
        }),
        Err(Problem::NotOneEraseUnit { region: "device_a" })
    );
}

/// The pairing sectors are named both or neither, each exactly one erase unit, as the device settings
/// are: a write of the pairings erases one whole sector. 8 KiB from 0x7F6000 runs into `pairing_b`
/// at 0x7F7000, and is refused as two units once `pairing_b` moves out of its way to the free sector
/// at 0xF000 (8 KiB from 0x7F6000 then ends at 0x7F8000, where `checkpoint_a` begins, and overlaps
/// nothing). Catches a pairing write that would erase a neighbouring region, and a file that names one
/// pairing sector, which a power cut during a write would leave without its previous tokens.
#[test]
fn the_pairings_are_named_in_pairs_of_one_erase_unit() {
    assert_eq!(
        c6(|file| remove(file, "layout", "pairing_a")),
        Err(Problem::MissingRegion {
            region: "pairing_a"
        })
    );
    assert_eq!(
        c6(|file| {
            set(file, "layout.pairing_b", "start", 0xF000_i64);
            set(file, "layout.pairing_a", "kib", 8_i64);
        }),
        Err(Problem::NotOneEraseUnit {
            region: "pairing_a"
        })
    );
}
