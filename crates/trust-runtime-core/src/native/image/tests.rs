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
}

impl Default for Sample {
    fn default() -> Self {
        Self {
            entry_offset: 16,
            state_offset: 0,
            sorted: true,
            without: None,
            release: false,
        }
    }
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

    let mut writer = ImageWriter::new(Target::X86_64, 1);
    writer.rustc = "1.99.0".into();
    writer.release = spec.release;
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
    let mut symbols = vec![
        SymbolRecord {
            canonical: vat,
            display: vat_display,
            type_index: 1,
            area: 6,
            flags: SYMBOL_PROGRAM,
            offset: 0,
            size: 4,
            parent: NONE,
            pou: 0,
        },
        SymbolRecord {
            canonical: batches,
            display: batches_display,
            type_index: 2,
            area: 5,
            flags: SYMBOL_RETAINED,
            offset: 0,
            size: 4,
            parent: 0,
            pou: 0,
        },
        SymbolRecord {
            canonical: state,
            display: state_display,
            type_index: 0,
            area: 6,
            flags: 0,
            offset: spec.state_offset,
            size: 2,
            parent: 0,
            pou: 0,
        },
    ];
    if !spec.sorted {
        symbols.swap(1, 2);
    }
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
            symbol: 1,
            offset: 0,
            size: 4,
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
    writer.section(SectionId::Strings, strings.into_bytes());
    let mut image = writer.finish().unwrap();
    if let Some(missing) = spec.without {
        remove_section(&mut image, missing);
    }
    image
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
    let mut version = bytes.clone();
    version[16] = 2;
    assert_eq!(
        Image::parse(&version).err(),
        Some(ImageError::FormatVersion(2))
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
