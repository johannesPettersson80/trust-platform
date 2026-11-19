//! The image format of specification 44 §5: what the build writes, the device reads where it lies,
//! and each check of §10.2 refuses with its own reason.

use alloc::vec;
use alloc::vec::Vec;

use super::*;

const MS: u64 = 1_000_000;

struct Sample {
    entry_offset: u32,
    state_offset: u32,
    sorted: bool,
    without: Option<SectionId>,
    release: bool,
    codesys: bool,
    /// With the crate table of [`SAMPLE_CRATES`] and an unsigned `SIGN`.
    crates: bool,
    /// With the Rust POU table of [`SAMPLE_RUST_POUS`] and an unsigned `SIGN`.
    rust_pous: bool,
    /// With a service table of these services and the paths each may write, and an unsigned
    /// `SIGN`; none when empty.
    services: &'static [(&'static str, &'static [&'static str])],
}

/// The sample with a service table of `services`, each with the paths it may write, for the service
/// table's tests (specification 44 §5.14).
pub(super) fn sample_with_services(
    services: &'static [(&'static str, &'static [&'static str])],
) -> Vec<u8> {
    sample(&Sample {
        services,
        ..Sample::default()
    })
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            entry_offset: 16,
            state_offset: 0,
            sorted: true,
            without: None,
            release: false,
            codesys: false,
            crates: false,
            rust_pous: false,
            services: &[],
        }
    }
}

/// The crates of a sample with a crate table, in their order: a path crate, a registry crate with
/// its archive's SHA-256, and a git crate with its commit (specification 44 §5.12).
const SAMPLE_CRATES: [(&str, &str, CrateSource, [u8; 32]); 3] = [
    ("helper", "0.3.1", CrateSource::Path, [0; 32]),
    ("libm", "0.2.16", CrateSource::Registry, [0xAB; 32]),
    ("trust-ext", "1.0.0", CrateSource::Git, git_commit(0xCD)),
];

/// The Rust POUs of a sample with a Rust POU table, in the order of their names in capitals, with
/// their budgets in nanoseconds (specification 44 §5.13): `FILTER` before its method
/// `FILTER.RESET`, whose name it begins.
const SAMPLE_RUST_POUS: [(&str, u64); 3] = [
    ("Clamp", 40_000),
    ("Filter", 1_500_000),
    ("Filter.Reset", 2_000),
];

/// A git crate's checksum: 20 bytes of commit, then 12 zero bytes.
const fn git_commit(byte: u8) -> [u8; 32] {
    let mut checksum = [0; 32];
    let mut index = 0;
    while index < 20 {
        checksum[index] = byte;
        index += 1;
    }
    checksum
}

/// A small image of one program `Vat` with a variable `State` in the instance area and a retained
/// `Batches`, run by `ControlTask` every 10 ms.
fn sample(spec: &Sample) -> Vec<u8> {
    let mut strings = StringPool::new();
    let vat = strings.add("VAT");
    let batches = strings.add("VAT.BATCHES");
    let state = strings.add("VAT.STATE");
    let vat_display = strings.add("Vat");
    let batches_display = strings.add("Vat.Batches");
    let state_display = strings.add("Vat.State");
    let task = strings.add("ControlTask");
    let file = strings.add("src/vat.st");
    let int = strings.add("INT");
    let program = strings.add("VatControl");
    let batches_key = strings.add("@program/Vat/Batches");

    let mut writer = ImageWriter::new(Target::X86_64, 1);
    writer.rustc = "1.99.0".into();
    writer.release = spec.release;
    if spec.codesys {
        writer.conversion_profile = ConversionProfile::Codesys;
    }
    writer.module(CodeImage {
        code: vec![0xC3; 32],
        rodata: vec![0x11; 16],
        rodata_distance: 40,
    });
    writer.table(
        &[
            EntryRecord {
                kind: EntryKind::ProgramBody as u32,
                index: 0,
                code_offset: 0,
            },
            EntryRecord {
                kind: EntryKind::ColdInit as u32,
                index: 0,
                code_offset: spec.entry_offset,
            },
        ],
        &[],
    );
    writer.table(
        &[
            AreaRecord {
                id: 5,
                length: 4,
                alignment: 4,
                flags: AREA_RETAINED | AREA_HAS_INIT,
                init_offset: 4,
            },
            AreaRecord {
                id: 6,
                length: 4,
                alignment: 2,
                flags: AREA_HAS_INIT,
                init_offset: 0,
            },
        ],
        &[],
    );
    writer.section(SectionId::Init, vec![7, 0, 0, 0, 3, 0, 0, 0]);
    writer.table(
        &[TaskRecord {
            name: task,
            interval_ns: 10 * MS,
            priority: 1,
            single: NONE,
            program_count: 1,
            program_first: 0,
            flags: 0,
        }],
        &0u32.to_le_bytes(),
    );
    writer.table::<IoBindRecord>(&[], &[]);
    let member = MemberDetail {
        name: state_display,
        type_index: 0,
        offset: 0,
        flags: 0,
    };
    let detail_offset = u32::try_from(TABLE_PREFIX_BYTES + 3 * TypeRecord::BYTES).unwrap();
    writer.table(
        &[
            TypeRecord {
                kind: type_kind::INT,
                name: int,
                size: 2,
                alignment: 2,
                detail: 0,
                count: 0,
            },
            TypeRecord {
                kind: type_kind::PROGRAM,
                name: program,
                size: 4,
                alignment: 2,
                detail: detail_offset,
                count: 1,
            },
            TypeRecord {
                kind: type_kind::UDINT,
                name: 0,
                size: 4,
                alignment: 4,
                detail: 0,
                count: 0,
            },
        ],
        &member.bytes(),
    );
    let symbols = sample_symbols(
        [vat, batches, state],
        [vat_display, batches_display, state_display],
        spec,
    );
    writer.table(&symbols, &[]);
    writer.table(
        &[StmtRecord {
            file: 0,
            pou: 0,
            start_line: 12,
            start_column: 1,
            end_line: 12,
            end_column: 20,
        }],
        &[],
    );
    writer.table(
        &[RetainRecord {
            key: batches_key,
            area: 5,
            offset: 0,
            type_index: 2,
        }],
        &[],
    );
    writer.table(
        &[FileRecord {
            label: file,
            source_offset: NONE,
            source_length: 0,
        }],
        &[],
    );
    writer.section(
        SectionId::MemFig,
        MemoryFigure {
            areas: 8,
            stack: 512,
            bookkeeping: 64,
            retain_capture: 8,
            total: 592,
            code: 32,
            rodata: 16,
            init: 8,
            call_level: 40,
        }
        .bytes()
        .to_vec(),
    );
    if spec.crates {
        let records: Vec<CrateRecord> = SAMPLE_CRATES
            .iter()
            .map(|(name, version, source, checksum)| CrateRecord {
                name: strings.add(name),
                version: strings.add(version),
                source: *source as u32,
                checksum: *checksum,
            })
            .collect();
        writer.table(&records, &[]);
        // Unsigned: algorithm 0, then a zero key and signature (§5.11).
        writer.section(SectionId::Sign, vec![0; 100]);
    }
    if spec.rust_pous {
        let records: Vec<RustPouRecord> = SAMPLE_RUST_POUS
            .iter()
            .map(|(name, budget_ns)| RustPouRecord {
                name: strings.add(name),
                reserved: 0,
                budget_ns: *budget_ns,
            })
            .collect();
        writer.table(&records, &[]);
        writer.section(SectionId::Sign, vec![0; 100]);
    }
    if !spec.services.is_empty() {
        service_table(&mut writer, &mut strings, spec.services);
    }
    writer.section(SectionId::Strings, strings.into_bytes());
    let mut image = writer.finish().unwrap();
    if let Some(missing) = spec.without {
        remove_section(&mut image, missing);
    }
    image
}

/// The service table of `services`, each with the paths it may write, and an unsigned `SIGN`
/// (specification 44 §5.14).
fn service_table(writer: &mut ImageWriter, strings: &mut StringPool, services: &[(&str, &[&str])]) {
    let mut records = Vec::new();
    let mut paths = Vec::new();
    for (name, writes) in services {
        records.push(ServiceRecord {
            name: strings.add(name),
            path_first: (paths.len() / 4) as u32,
            path_count: writes.len() as u32,
            reserved: 0,
        });
        for path in *writes {
            paths.extend_from_slice(&strings.add(path).to_le_bytes());
        }
    }
    writer.table(&records, &paths);
    writer.section(SectionId::Sign, vec![0; 100]);
}

/// The sample's symbols, by their canonical and display names' string offsets: the program `Vat`, its
/// retained `Batches` and its `State` at `spec.state_offset`, in canonical-name order unless
/// `spec.sorted` is false.
fn sample_symbols(canonical: [u32; 3], display: [u32; 3], spec: &Sample) -> Vec<SymbolRecord> {
    let mut symbols = vec![
        SymbolRecord {
            canonical: canonical[0],
            display: display[0],
            type_index: 1,
            area: 6,
            flags: SYMBOL_PROGRAM,
            noinit: false,
            hidden: false,
            owner: 0,
            offset: 0,
            size: 4,
            parent: NONE,
            pou: 0,
        },
        SymbolRecord {
            canonical: canonical[1],
            display: display[1],
            type_index: 2,
            area: 5,
            flags: SYMBOL_RETAINED,
            noinit: false,
            hidden: false,
            owner: 0,
            offset: 0,
            size: 4,
            parent: 0,
            pou: 0,
        },
        SymbolRecord {
            canonical: canonical[2],
            display: display[2],
            type_index: 0,
            area: 6,
            flags: 0,
            noinit: false,
            hidden: false,
            owner: 0,
            offset: spec.state_offset,
            size: 2,
            parent: 0,
            pou: 0,
        },
    ];
    if !spec.sorted {
        symbols.swap(1, 2);
    }
    symbols
}

/// Renames section `id` in the section table to an unused slot by turning it into a second
/// `SOURCE`-free gap: the entry gets id 14 (`SOURCE`, optional) and the CRC is recomputed, so only
/// the missing section is wrong.
fn remove_section(image: &mut [u8], id: SectionId) {
    let count = u32::from_le_bytes(image[112..116].try_into().unwrap()) as usize;
    for index in 0..count {
        let entry = HEADER_BYTES + index * SECTION_ENTRY_BYTES;
        if u32::from_le_bytes(image[entry..entry + 4].try_into().unwrap()) == id as u32 {
            image[entry..entry + 4].copy_from_slice(&(SectionId::Source as u32).to_le_bytes());
        }
    }
    let end = HEADER_BYTES + count * SECTION_ENTRY_BYTES;
    let crc = crate::crc32::checksum(&image[12..end]);
    image[8..12].copy_from_slice(&crc.to_le_bytes());
}

/// What the build writes the device reads back: the header, the module with `RODATA` 40 bytes after
/// `CODE` at a 64-byte boundary, the tables, the strings, the task's program list and a symbol by
/// one binary search of its canonical name.
#[test]
fn an_image_reads_back_as_written() {
    let bytes = sample(&Sample::default());
    let image = Image::parse(&bytes).unwrap();
    image.check_tables().unwrap();
    assert_eq!(image.header().target, Target::X86_64 as u32);
    assert_eq!(image.header().interface_version, 1);
    assert_eq!(image.header().rustc_version(), "1.99.0");
    let (code_offset, module) = image.module();
    assert_eq!(code_offset as usize % CODE_ALIGNMENT, 0);
    assert_eq!(module.len(), 40 + 16);
    assert_eq!(&module[..32], &[0xC3; 32]);
    assert_eq!(&module[32..40], &[0; 8]);
    assert_eq!(&module[40..], &[0x11; 16]);
    let entries = image.table::<EntryRecord>().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.get(1).unwrap().kind, EntryKind::ColdInit as u32);
    let tasks = image.table::<TaskRecord>().unwrap();
    let task = tasks.get(0).unwrap();
    assert_eq!(image.string(task.name), Some("ControlTask"));
    assert_eq!(task.interval_ns, 10 * MS);
    assert_eq!(image.task_programs(&task).collect::<Vec<_>>(), [0]);
    let (index, state) = image.find_symbol("VAT.STATE").unwrap();
    assert_eq!(index, 2);
    assert_eq!(image.string(state.display), Some("Vat.State"));
    assert_eq!(image.find_symbol("VAT.MISSING"), None);
    assert_eq!(image.find_symbol("VAT").unwrap().0, 0);
    assert_eq!(image.area_length(5), Some(4));
    assert_eq!(
        image.section_bytes(SectionId::Init),
        &[7, 0, 0, 0, 3, 0, 0, 0]
    );
    assert_eq!(image.memory_figure().total, 592);
    assert_eq!(image.memory_figure().call_level, 40);
    let types = image.table::<TypeRecord>().unwrap();
    let program = types.get(1).unwrap();
    let detail = &image.section_bytes(SectionId::Types)[program.detail as usize..];
    let member = MemberDetail::read(&detail[..MEMBER_BYTES]);
    assert_eq!(image.string(member.name), Some("Vat.State"));
}

/// The same tables give the same bytes (specification 44 NX-07): the writer adds nothing that
/// varies.
#[test]
fn an_image_is_written_deterministically() {
    assert_eq!(sample(&Sample::default()), sample(&Sample::default()));
}

/// Bit 2 of the header flags says that the program converts with the CODESYS rules of specification
/// 07 §2.10 (specification 44 §5.2); an image without it converts with the IEC rules. An online
/// change between images of other rules is refused with both rules named, one between images of
/// the same rules is not (specification 07 §2.10: "an online change does not switch them").
/// Catches the flag at another bit, the rules read from another flag, and a change that switches
/// the rules of a running program.
#[test]
fn the_conversion_rules_are_bit_2_of_the_header_flags() {
    let iec = sample(&Sample::default());
    let codesys = sample(&Sample {
        codesys: true,
        ..Sample::default()
    });
    assert_eq!(&iec[120..124], &[0; 4]);
    assert_eq!(&codesys[120..124], &[4, 0, 0, 0]);
    let (iec, codesys) = (Image::parse(&iec).unwrap(), Image::parse(&codesys).unwrap());
    assert_eq!(iec.header().conversion_profile(), ConversionProfile::Iec);
    assert_eq!(
        codesys.header().conversion_profile(),
        ConversionProfile::Codesys
    );
    assert_eq!(same_conversion_rules(iec.header(), iec.header()), Ok(()));
    assert_eq!(
        same_conversion_rules(codesys.header(), codesys.header()),
        Ok(())
    );
    for (running, changed, to, from) in [
        (&iec, &codesys, "codesys", "iec"),
        (&codesys, &iec, "iec", "codesys"),
    ] {
        let Err(RuntimeError::InvalidConfig(reason)) =
            same_conversion_rules(running.header(), changed.header())
        else {
            panic!("a change from the {from} rules to the {to} rules is refused");
        };
        assert!(
            reason.contains(&alloc::format!("with the {to} conversion rules"))
                && reason.contains(&alloc::format!("running program with the {from} rules")),
            "{reason}"
        );
    }
}

/// A release image says so in bit 0 of the header flags at offset 120, and a debug image leaves the
/// flags zero (specification 44 §5.2, §13.2). The flags lie under the header CRC, so a debug image
/// whose flag is set afterwards is refused as damaged rather than read as a release image.
#[test]
fn a_release_image_says_so_in_its_header_flags() {
    let debug = sample(&Sample::default());
    assert_eq!(&debug[120..128], &[0; 8]);
    assert!(!Image::parse(&debug).unwrap().header().is_release());

    let release = sample(&Sample {
        release: true,
        ..Sample::default()
    });
    assert_eq!(&release[120..124], &[1, 0, 0, 0]);
    assert_eq!(&release[124..128], &[0; 4]);
    let image = Image::parse(&release).unwrap();
    image.check_tables().unwrap();
    assert!(image.header().is_release());
    assert_eq!(image.header().flags, FLAG_RELEASE);

    let mut altered = debug;
    altered[120] = 1;
    assert!(matches!(
        Image::parse(&altered),
        Err(ImageError::Crc { .. })
    ));
}

/// Each damage is refused with its own reason before anything else is read.
#[test]
fn a_damaged_image_is_refused_with_its_reason() {
    let bytes = sample(&Sample::default());
    assert_eq!(
        Image::parse(&bytes[..100]).err(),
        Some(ImageError::TooShort { length: 100 })
    );
    let mut magic = bytes.clone();
    magic[7] = b'2';
    assert_eq!(Image::parse(&magic).err(), Some(ImageError::Magic));
    let truncated = &bytes[..bytes.len() - 8];
    assert!(matches!(
        Image::parse(truncated),
        Err(ImageError::Length { .. })
    ));
    // An image of format 1, before the retain map named an area for each variable (specification
    // 44 0.13), is refused.
    let mut version = bytes.clone();
    version[16] = 1;
    assert_eq!(
        Image::parse(&version).err(),
        Some(ImageError::FormatVersion(1))
    );
    // An image of format 5, which release 0.43.0 builds, before the Rust POU table (0.58), is
    // refused with both versions, never as an image of version 1: catches a reader that takes
    // the older format and a refusal that names another version than the one it reads.
    version[16] = 5;
    let refusal = Image::parse(&version).err();
    assert_eq!(refusal, Some(ImageError::FormatVersion(5)));
    assert_eq!(
        refusal.map(|error| error.to_string()).as_deref(),
        Some("image format version 5, this runtime reads 6: rebuild the project")
    );
    let mut table = bytes.clone();
    table[HEADER_BYTES + 12] ^= 1;
    assert!(matches!(Image::parse(&table), Err(ImageError::Crc { .. })));
    let missing = sample(&Sample {
        without: Some(SectionId::Retain),
        ..Sample::default()
    });
    assert_eq!(
        Image::parse(&missing).err(),
        Some(ImageError::MissingSection(SectionId::Retain))
    );
}

/// A record that names something outside its section or area is refused by the table check
/// (specification 44 §10.2, step 5): an entry 64 bytes into a 32-byte `CODE`, a variable at offset 4
/// with 2 bytes in a 4-byte area, symbols out of name order.
#[test]
fn a_record_outside_its_bounds_is_refused() {
    let entry = sample(&Sample {
        entry_offset: 64,
        ..Sample::default()
    });
    assert_eq!(
        Image::parse(&entry).unwrap().check_tables().err(),
        Some(ImageError::Record {
            section: SectionId::Entry,
            index: 1,
            reason: "entry outside CODE",
        })
    );
    let variable = sample(&Sample {
        state_offset: 4,
        ..Sample::default()
    });
    assert_eq!(
        Image::parse(&variable).unwrap().check_tables().err(),
        Some(ImageError::Record {
            section: SectionId::Symbols,
            index: 2,
            reason: "variable outside its area",
        })
    );
    let unsorted = sample(&Sample {
        sorted: false,
        ..Sample::default()
    });
    assert_eq!(
        Image::parse(&unsorted).unwrap().check_tables().err(),
        Some(ImageError::Record {
            section: SectionId::Symbols,
            index: 2,
            reason: "not sorted by canonical name",
        })
    );
}

/// `RODATA` that would overlap `CODE` cannot be written.
#[test]
fn rodata_cannot_overlap_code() {
    let mut writer = ImageWriter::new(Target::Rv32imac, 1);
    writer.module(CodeImage {
        code: vec![0; 40],
        rodata: vec![0; 8],
        rodata_distance: 32,
    });
    assert_eq!(writer.finish().err(), Some(WriteError::RodataPlacement));
}

/// The pool holds each string once and the empty string at 0.
#[test]
fn the_string_pool_holds_each_string_once() {
    let mut pool = StringPool::new();
    assert_eq!(pool.add(""), 0);
    let first = pool.add("Vat");
    assert_eq!(first, 1);
    assert_eq!(pool.add("Vat"), first);
    assert_eq!(pool.add("Plant"), 5);
    assert_eq!(pool.into_bytes(), b"\0Vat\0Plant\0");
}

/// Specification 44 §5.7: a symbol's `noinit` mark is bit 0 of its record's byte 14, the byte after
/// its flags, and byte 15 holds the owner, none here; a record reads back with its mark. Catches the
/// mark in another byte or bit, which an image of another truST would read as some other flag or
/// none.
#[test]
fn a_symbols_noinit_mark_is_bit_0_of_its_byte_14() {
    for noinit in [false, true] {
        let symbol = SymbolRecord {
            canonical: 1,
            display: 2,
            type_index: 3,
            area: 5,
            flags: SYMBOL_RETAINED,
            noinit,
            hidden: false,
            owner: 0,
            offset: 8,
            size: 4,
            parent: NONE,
            pou: NONE,
        };
        let mut bytes = [0u8; SymbolRecord::BYTES];
        symbol.write(&mut bytes);
        assert_eq!(bytes[12..16], [5, SYMBOL_RETAINED, u8::from(noinit), 0]);
        assert_eq!(SymbolRecord::read(&bytes), symbol);
    }
}

/// Specification 44 §5.7: a symbol's `hide` mark is bit 1 of its record's byte 14, beside the
/// `noinit` mark at bit 0, and byte 15 holds the owner, none here; a record reads back with both
/// marks. Catches the mark in another byte or bit, or one mark read as the other.
#[test]
fn a_symbols_hide_mark_is_bit_1_of_its_byte_14() {
    for (noinit, hidden, marks) in [(false, true, 2u8), (true, true, 3), (true, false, 1)] {
        let symbol = SymbolRecord {
            canonical: 1,
            display: 2,
            type_index: 3,
            area: 5,
            flags: SYMBOL_RETAINED,
            noinit,
            hidden,
            owner: 0,
            offset: 8,
            size: 4,
            parent: NONE,
            pou: NONE,
        };
        let mut bytes = [0u8; SymbolRecord::BYTES];
        symbol.write(&mut bytes);
        assert_eq!(bytes[12..16], [5, SYMBOL_RETAINED, marks, 0]);
        assert_eq!(SymbolRecord::read(&bytes), symbol);
    }
}

/// Specification 44 §5.7: a global's owner is its record's byte 15, after the marks of byte 14: 0 for
/// none, a task's index in the task table plus one (1 for the first task, 127 for the last that the
/// byte names), 128 for a global no program writes and 129 for one only programs outside every task
/// write; a record reads back with it. Catches the owner in another byte, the task's index written
/// without the plus one (the first task would read as no owner), and the two kinds swapped.
#[test]
fn a_globals_owner_is_its_byte_15() {
    assert_eq!(
        (
            OWNER_UNKNOWN,
            OWNER_LAST_TASK,
            OWNER_UNWRITTEN,
            OWNER_BACKGROUND
        ),
        (0, 127, 128, 129)
    );
    for owner in [
        OWNER_UNKNOWN,
        1,
        OWNER_LAST_TASK,
        OWNER_UNWRITTEN,
        OWNER_BACKGROUND,
    ] {
        let symbol = SymbolRecord {
            canonical: 1,
            display: 2,
            type_index: 3,
            area: 5,
            flags: SYMBOL_RETAINED,
            noinit: true,
            hidden: false,
            owner,
            offset: 8,
            size: 4,
            parent: NONE,
            pou: NONE,
        };
        let mut bytes = [0u8; SymbolRecord::BYTES];
        symbol.write(&mut bytes);
        assert_eq!(bytes[12..16], [5, SYMBOL_RETAINED, 1, owner]);
        assert_eq!(SymbolRecord::read(&bytes), symbol);
    }
}

/// The crate table (specification 44 §5.12): the writer puts it before `SIGN`, the reader gives each
/// crate's name, version, source and checksum back in their order, and the image's deployment record
/// carries them (§10.1); an image without the table has no crate. Format version 5 named it first,
/// and version 6 keeps it. Catches a table after the signature, which would leave the list
/// unsigned, a crate read from the wrong bytes, and a record that loses the list.
#[test]
fn the_crate_table_reads_back_before_sign_and_into_the_record() {
    let bytes = sample(&Sample {
        crates: true,
        ..Sample::default()
    });
    let image = Image::parse(&bytes).unwrap();
    image.check_tables().unwrap();
    assert_eq!(image.header().format_version, 6);
    let crates = image.section(SectionId::Crates).expect("the crate table");
    let sign = image.section(SectionId::Sign).expect("the signature");
    assert!(crates.offset < sign.offset, "CRATES lies before SIGN");
    assert_eq!(sign.end(), bytes.len() as u64, "SIGN is last");
    let table = image.table::<CrateRecord>().unwrap();
    let read: Vec<_> = table
        .iter()
        .map(|entry| {
            (
                image.string(entry.name).unwrap(),
                image.string(entry.version).unwrap(),
                CrateSource::from_raw(entry.source).unwrap(),
                entry.checksum,
            )
        })
        .collect();
    assert_eq!(read, SAMPLE_CRATES);

    let record = crate::deployment::DeploymentRecord::for_image(&bytes, [0; 32]).unwrap();
    let listed: Vec<_> = record
        .crates
        .iter()
        .map(|entry| {
            (
                entry.name.as_str(),
                entry.version.as_str(),
                entry.source,
                entry.checksum,
            )
        })
        .collect();
    assert_eq!(listed, SAMPLE_CRATES);
    let without = sample(&Sample::default());
    assert_eq!(
        Image::parse(&without).unwrap().section(SectionId::Crates),
        None
    );
    assert!(
        crate::deployment::DeploymentRecord::for_image(&without, [0; 32])
            .unwrap()
            .crates
            .is_empty()
    );
}

/// Step 5 of §10.2 refuses a crate table that the build cannot have written: a name or version
/// outside the string pool or empty, an unknown source, a checksum byte that its source leaves zero,
/// crates out of order, and a count its section does not hold. Catches a device that signs off a
/// list it cannot read.
#[test]
fn a_malformed_crate_table_is_refused() {
    let bytes = sample(&Sample {
        crates: true,
        ..Sample::default()
    });
    let records = Image::parse(&bytes)
        .unwrap()
        .section(SectionId::Crates)
        .unwrap()
        .offset as usize
        + TABLE_PREFIX_BYTES;
    let at = |index: usize, field: usize| records + index * CrateRecord::BYTES + field;
    let refused = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut edited = bytes.clone();
        edit(&mut edited);
        Image::parse(&edited)
            .and_then(|image| image.check_tables())
            .expect_err("the table is refused")
    };
    let crate_record = |index, reason| ImageError::Record {
        section: SectionId::Crates,
        index,
        reason,
    };
    assert_eq!(
        refused(&|image| image[at(1, 8)] = 4),
        crate_record(1, "unknown source")
    );
    assert_eq!(
        refused(&|image| image[at(0, 12 + 31)] = 1),
        crate_record(0, "checksum bytes its source leaves zero are not zero")
    );
    assert_eq!(
        refused(&|image| image[at(2, 12 + 20)] = 1),
        crate_record(2, "checksum bytes its source leaves zero are not zero")
    );
    assert_eq!(
        refused(&|image| image[at(1, 0)..at(1, 4)].copy_from_slice(&u32::MAX.to_le_bytes())),
        crate_record(1, "name or version not a string of STRINGS")
    );
    assert_eq!(
        refused(&|image| image[at(2, 4)..at(2, 8)].fill(0)),
        crate_record(2, "name or version not a string of STRINGS")
    );
    // libm takes helper's name: two crates named helper, 0.2.16 before 0.3.1.
    assert_eq!(
        refused(&|image| {
            let helper = image[at(0, 0)..at(0, 4)].to_vec();
            image[at(1, 0)..at(1, 4)].copy_from_slice(&helper);
        }),
        crate_record(1, "out of order or repeated")
    );
    assert_eq!(
        refused(&|image| image[records - TABLE_PREFIX_BYTES] = 200),
        ImageError::Table(SectionId::Crates)
    );
}

/// The Rust POU table (specification 44 §5.13): the writer puts it before `SIGN`, and the reader
/// gives each POU's name and budget back in their order; an image without the table, a release
/// image's, has none. Catches a table after the signature, which would leave the budgets unsigned,
/// and a budget read from the wrong bytes.
#[test]
fn the_rust_pou_table_reads_back_before_sign() {
    let bytes = sample(&Sample {
        rust_pous: true,
        ..Sample::default()
    });
    let image = Image::parse(&bytes).unwrap();
    image.check_tables().unwrap();
    let pous = image
        .section(SectionId::RustPous)
        .expect("the Rust POU table");
    let sign = image.section(SectionId::Sign).expect("the signature");
    assert!(pous.offset < sign.offset, "RUSTPOUS lies before SIGN");
    assert_eq!(sign.end(), bytes.len() as u64, "SIGN is last");
    let read: Vec<_> = image
        .table::<RustPouRecord>()
        .unwrap()
        .iter()
        .map(|entry| (image.string(entry.name).unwrap(), entry.budget_ns))
        .collect();
    assert_eq!(read, SAMPLE_RUST_POUS);
    let without = sample(&Sample::default());
    assert_eq!(
        Image::parse(&without).unwrap().section(SectionId::RustPous),
        None
    );
}

/// Step 5 of §10.2 refuses a Rust POU table that the build cannot have written: a name outside the
/// string pool or empty, a reserved word that is not 0, a budget of 0 or past `i64::MAX`, POUs out
/// of the order of their names in capitals or repeated, and a count its section does not hold.
/// Catches a kernel that numbers the records of a table it cannot read.
#[test]
fn a_malformed_rust_pou_table_is_refused() {
    let bytes = sample(&Sample {
        rust_pous: true,
        ..Sample::default()
    });
    let records = Image::parse(&bytes)
        .unwrap()
        .section(SectionId::RustPous)
        .unwrap()
        .offset as usize
        + TABLE_PREFIX_BYTES;
    let at = |index: usize, field: usize| records + index * RustPouRecord::BYTES + field;
    let refused = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut edited = bytes.clone();
        edit(&mut edited);
        Image::parse(&edited)
            .and_then(|image| image.check_tables())
            .expect_err("the table is refused")
    };
    let pou_record = |index, reason| ImageError::Record {
        section: SectionId::RustPous,
        index,
        reason,
    };
    assert_eq!(
        refused(&|image| image[at(0, 0)..at(0, 4)].copy_from_slice(&u32::MAX.to_le_bytes())),
        pou_record(0, "name not a string of STRINGS")
    );
    assert_eq!(
        refused(&|image| image[at(1, 0)..at(1, 4)].fill(0)),
        pou_record(1, "name not a string of STRINGS")
    );
    assert_eq!(
        refused(&|image| image[at(2, 4)] = 1),
        pou_record(2, "reserved word not 0")
    );
    assert_eq!(
        refused(&|image| image[at(1, 8)..at(1, 16)].fill(0)),
        pou_record(1, "budget not from 1 ns to i64::MAX ns")
    );
    assert_eq!(
        refused(&|image| image[at(0, 8)..at(0, 16)].copy_from_slice(&u64::MAX.to_le_bytes())),
        pou_record(0, "budget not from 1 ns to i64::MAX ns")
    );
    // Filter takes Clamp's name: two POUs named Clamp.
    assert_eq!(
        refused(&|image| {
            let clamp = image[at(0, 0)..at(0, 4)].to_vec();
            image[at(1, 0)..at(1, 4)].copy_from_slice(&clamp);
        }),
        pou_record(1, "out of order or repeated")
    );
    // Filter.Reset takes Clamp's name, which sorts before Filter's.
    assert_eq!(
        refused(&|image| {
            let clamp = image[at(0, 0)..at(0, 4)].to_vec();
            image[at(2, 0)..at(2, 4)].copy_from_slice(&clamp);
        }),
        pou_record(2, "out of order or repeated")
    );
    assert_eq!(
        refused(&|image| image[records - TABLE_PREFIX_BYTES] = 200),
        ImageError::Table(SectionId::RustPous)
    );
}
