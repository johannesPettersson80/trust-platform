//! The boot counter's two sectors in a chip file (specification 34 Appendix H.2 and H.5 rule 7).
//! Each test edits a real chip file and checks that the edit is refused with the problem of the
//! broken rule, or that an edit within the rules is accepted.

use trust_chips::{ChipFile, Problem};

const U5A9: &str = include_str!("../../../chips/stm32u5a9.toml");
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

fn u5a9(edit: impl FnOnce(&mut toml::Table)) -> Result<ChipFile, Problem> {
    edited("stm32u5a9", U5A9, edit)
}

fn c6(edit: impl FnOnce(&mut toml::Table)) -> Result<ChipFile, Problem> {
    edited("esp32c6", C6, edit)
}

/// The boot counter's two sectors are named both or neither, and each is exactly one erase unit
/// (specification 34 Appendix H.2 and H.5 rule 7). On the C6 the erase unit is 4 KiB, so the real
/// file's two sectors, 0x9000 and 0xA000, are one unit each. Catches a file that gives the counter
/// one sector, a sector of two units (the counter erases its whole sector when its log is full,
/// so a second unit would be erased with it), half a unit, a sector that starts off a unit, and an
/// empty sector.
#[test]
fn the_boot_counter_is_named_in_pairs_of_exactly_one_erase_unit_each() {
    assert_eq!(
        c6(|file| remove(file, "layout", "boot_counter_b")),
        Err(Problem::MissingRegion {
            region: "boot_counter_b"
        })
    );
    assert_eq!(
        c6(|file| remove(file, "layout", "boot_counter_a")),
        Err(Problem::MissingRegion {
            region: "boot_counter_a"
        })
    );
    // A file that names neither belongs to a chip that keeps its count elsewhere.
    let neither = c6(|file| {
        remove(file, "layout", "boot_counter_a");
        remove(file, "layout", "boot_counter_b");
    });
    assert_eq!(neither.err(), None);
    // 8 KiB from 0x9000 end at 0xB000, two units. B moves to 0xC000 so that the two stay apart, and
    // 8 KiB from 0xC000 end at 0xE000, below the firmware at 0x1_0000.
    assert_eq!(
        c6(|file| {
            set(file, "layout.boot_counter_a", "kib", 8_i64);
            set(file, "layout.boot_counter_b", "start", 0xC000_i64);
        }),
        Err(Problem::NotOneEraseUnit {
            region: "boot_counter_a"
        })
    );
    assert_eq!(
        c6(|file| {
            set(file, "layout.boot_counter_b", "start", 0xC000_i64);
            set(file, "layout.boot_counter_b", "kib", 8_i64);
        }),
        Err(Problem::NotOneEraseUnit {
            region: "boot_counter_b"
        })
    );
    // 2 KiB from 0x9000 end at 0x9800, in the middle of a unit; a region from 0xA800 starts in the
    // middle of one.
    assert_eq!(
        c6(|file| set(file, "layout.boot_counter_a", "kib", 2_i64)),
        Err(Problem::RegionNotAligned {
            region: "boot_counter_a"
        })
    );
    assert_eq!(
        c6(|file| set(file, "layout.boot_counter_b", "start", 0xA800_i64)),
        Err(Problem::RegionNotAligned {
            region: "boot_counter_b"
        })
    );
    assert_eq!(
        c6(|file| set(file, "layout.boot_counter_a", "kib", 0_i64)),
        Err(Problem::ZeroSize {
            what: "layout.boot_counter_a".to_owned()
        })
    );
}

/// A boot counter sector overlaps no other region (Appendix H.5 rule 7). The C6's layout, in the
/// order of the format, is the boot loader from 0x0 to 0x8000, the partition table to 0x9000, the
/// firmware from 0x10000 to 0x200000, the application to 0x7F8000 and the checkpoint slots from
/// 0x7F8000 and 0x7FC000, 16 KiB each. A counter sector put on the first 4 KiB of each of them
/// is refused, and the refusal names that region and the sector. Catches a counter that could be
/// written over one of them, a layout that later reuses the counter's sectors, and a check that
/// compares a counter sector with the application only.
#[test]
fn a_boot_counter_sector_overlapping_another_region_is_refused() {
    for (other, start) in [
        ("bootloader", 0x7000_i64),
        ("partition_table", 0x8000),
        ("firmware", 0x1_0000),
        ("application", 0x20_0000),
        ("checkpoint_a", 0x7F_8000),
        ("checkpoint_b", 0x7F_C000),
    ] {
        assert_eq!(
            c6(|file| set(file, "layout.boot_counter_a", "start", start)),
            Err(Problem::RegionsOverlap {
                first: other,
                second: "boot_counter_a"
            }),
            "{other}"
        );
        assert_eq!(
            c6(|file| set(file, "layout.boot_counter_b", "start", start)),
            Err(Problem::RegionsOverlap {
                first: other,
                second: "boot_counter_b"
            }),
            "{other}"
        );
    }
    // The second sector on the first.
    assert_eq!(
        c6(|file| set(file, "layout.boot_counter_b", "start", 0x9000_i64)),
        Err(Problem::RegionsOverlap {
            first: "boot_counter_a",
            second: "boot_counter_b"
        })
    );
    // A later layout that grows the firmware down into the gap: from 0x9000 to where it ended, at
    // 0x20_0000, is 0x1F_7000 bytes, 2,012 KiB.
    assert_eq!(
        c6(|file| {
            set(file, "layout.firmware", "start", 0x9000_i64);
            set(file, "layout.firmware", "kib", 2012_i64);
        }),
        Err(Problem::RegionsOverlap {
            first: "firmware",
            second: "boot_counter_a"
        })
    );
}

/// On flash in the address space the erase unit is the sector at the region's start (Appendix H.5
/// rule 7). The U5A9's flash is 512 pages of 8 KiB from 0x08000000 to 0x08400000. With the firmware
/// cut from 2,040 to 2,008 KiB, so that it ends at 0x081F8000, the four pages at 0x081F8000,
/// 0x081FA000, 0x081FC000 and 0x081FE000 are free below the application at 0x08200000, and one page
/// is one erase unit. Catches a check that takes a fixed 4 KiB unit, and one that accepts several
/// pages, part of a page, a page off its boundary or a page past the end of the flash.
#[test]
fn a_boot_counter_sector_on_mapped_flash_is_exactly_one_sector() {
    let counters = |a: (u32, u32), b: (u32, u32)| {
        move |file: &mut toml::Table| {
            set(file, "layout.firmware", "kib", 2008_i64);
            for (key, (start, kib)) in [("boot_counter_a", a), ("boot_counter_b", b)] {
                let region = table(&format!("start = {start}\nkib = {kib}"));
                set(file, "layout", key, toml::Value::Table(region));
            }
        }
    };
    let (first, third) = ((0x081F_8000, 8), (0x081F_C000, 8));
    assert_eq!(u5a9(counters(first, third)).err(), None);
    // The third and fourth pages together end at 0x0820_0000, where the application begins:
    // aligned, apart from everything, and two units.
    assert_eq!(
        u5a9(counters(first, (0x081F_C000, 16))),
        Err(Problem::NotOneEraseUnit {
            region: "boot_counter_b"
        })
    );
    // 4 KiB from 0x081F_C000 end in the middle of the third page; 8 KiB from 0x081F_D000 start in
    // the middle of it.
    assert_eq!(
        u5a9(counters(first, (0x081F_C000, 4))),
        Err(Problem::RegionNotAligned {
            region: "boot_counter_b"
        })
    );
    assert_eq!(
        u5a9(counters(first, (0x081F_D000, 8))),
        Err(Problem::RegionNotAligned {
            region: "boot_counter_b"
        })
    );
    // 12 KiB from 0x081F_8000 end at 0x081F_B000, in the middle of the second page.
    assert_eq!(
        u5a9(counters((0x081F_8000, 12), third)),
        Err(Problem::RegionNotAligned {
            region: "boot_counter_a"
        })
    );
    // The page after the last one, 0x0840_0000, is past the end of the 4 MiB flash.
    assert_eq!(
        u5a9(counters(first, (0x0840_0000, 8))),
        Err(Problem::RegionOutsideFlash {
            region: "boot_counter_b"
        })
    );
}
