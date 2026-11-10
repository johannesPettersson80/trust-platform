//! The program image's header names what it was built from (specification 44 §5.2): the hash of its
//! sources and the program library it links, which a kernel of the same release admits (§10.2). Its
//! run tables hold its entries, tasks, I/O bindings and statements (§5.4, §5.6, §5.9).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};

/// A counter in a 10 ms task.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

/// Specification 44 §5.2, worked with Python's `hashlib`: the sources hash is the SHA-256 of each
/// source's label, a zero byte, its text and a zero byte, in the order of the labels:
/// `src/a.st\0A\0src/b.st\0B\0` for the sources given as `src/b.st` then `src/a.st`. Catches the
/// sources hashed in the order given, the labels left out, and texts run together.
#[test]
fn the_sources_hash_takes_each_label_and_text_in_label_order() {
    let sources = [
        SourceFile::with_path("src/b.st", "B"),
        SourceFile::with_path("src/a.st", "A"),
    ];
    assert_eq!(
        trust_runtime::native::sources_hash(&sources),
        [
            0xa0, 0x61, 0x19, 0xe9, 0xc7, 0x6c, 0xcb, 0x9d, 0x73, 0x48, 0xdc, 0xdb, 0x20, 0xa2,
            0x90, 0x2e, 0x3f, 0x8d, 0xbc, 0xcf, 0x68, 0x33, 0x30, 0x1f, 0xb6, 0x0d, 0xed, 0x7b,
            0xdb, 0x39, 0xc3, 0x28
        ]
    );
}

/// Specification 44 §5.2 and §10.2, step 2: an image built with this release's toolchain names
/// its sources' hash and this release's program library, which this release's kernel admits.
/// Catches a header left at zeros, which no kernel admits, and a library hash of another release.
#[test]
fn a_built_image_names_its_sources_and_this_releases_program_library() {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let sources = [SourceFile::with_path("src/counter.st", COUNTER)];
    let host = Target::this_cpu().expect("a computer target");
    let work = trust_test_support::unique_temp_path(&format!(
        "trust-native-header-{}",
        std::process::id()
    ));
    let built = build(
        &sources,
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the counter builds");
    let _ = std::fs::remove_dir_all(&work);
    let image =
        trust_runtime_core::native::image::Image::parse(&built.image).expect("the image parses");
    assert_eq!(
        image.header().source_hash,
        trust_runtime::native::sources_hash(&sources)
    );
    assert_eq!(
        image.header().library,
        trust_runtime_core::native::library::library_hash(
            trust_runtime_core::native::library::PROGRAM_LIBRARY,
            host
        )
    );
}

/// Two programs: `Main` in the 10 ms task `Fast`, whose `Counter` counts while its input `Start` at
/// `%IX0.1` is TRUE, and `Background`, an `Idle` that no task runs.
const TABLES: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
    Start AT %IX0.1 : BOOL;
END_VAR
IF Start THEN
    Count := Count + 1;
END_IF
END_PROGRAM

PROGRAM Idle
VAR
    Ticks : UINT;
END_VAR
Ticks := Ticks + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
    PROGRAM Background : Idle;
END_RESOURCE
END_CONFIGURATION
";

/// Specification 44 §5.4, §5.6 and §5.9, worked by hand for `TABLES` (its first line is empty):
/// - the entry table holds each program instance's body, `Main` (instance 0) and `Background`
///   (instance 1), and the cold and the warm initialization, with index 0; no interface, so no
///   entry for tools;
/// - the task table holds `Fast` (10,000,000 ns, priority 1, no `SINGLE`, one program: Main's body)
///   and the record of the programs without a task (priority 0xFFFF_FFFF, one program: Background's
///   body);
/// - `IOBIND` holds one record, `Main.Start` at `%IX0.1`;
/// - the statement table holds the three statements in source order, all in file 0: the `IF` at line
///   7, column 1, to its `END_IF` on line 9; the assignment inside it at line 8, column 5; and
///   `Idle`'s at line 16, column 1; the first two in one POU, the third in another;
/// - the one file is labelled `src/probe.st`.
///
/// Catches a run table lost or garbled by its writer: a task that lists another program's entry, the
/// programs without a task left out, a binding to another symbol or address, and statements out of
/// source order or in another file.
#[test]
fn a_built_image_holds_its_entries_tasks_bindings_and_statements() {
    use trust_runtime_core::io_address::IoAddress;
    use trust_runtime_core::native::image::{
        EntryRecord, FileRecord, Image, IoBindRecord, StmtRecord, TaskRecord, NONE,
    };
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-tables-{}", std::process::id()));
    let built = build(
        &[SourceFile::with_path("src/probe.st", TABLES)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the probe builds");
    let _ = std::fs::remove_dir_all(&work);
    let image = Image::parse(&built.image).expect("the image parses");

    let entries: Vec<(u32, u32)> = image
        .table::<EntryRecord>()
        .expect("the entry table")
        .iter()
        .map(|entry| (entry.kind, entry.index))
        .collect();
    assert_eq!(entries, [(1, 0), (1, 1), (2, 0), (3, 0)]);
    let body = |instance: u32| {
        u32::try_from(
            entries
                .iter()
                .position(|&entry| entry == (1, instance))
                .expect("the program's body entry"),
        )
        .unwrap()
    };

    let tasks: Vec<TaskRecord> = image
        .table::<TaskRecord>()
        .expect("the task table")
        .iter()
        .collect();
    assert_eq!(tasks.len(), 2);
    let fast = tasks
        .iter()
        .find(|task| image.string(task.name) == Some("Fast"))
        .expect("the task Fast");
    assert_eq!(
        (
            fast.interval_ns,
            fast.priority,
            fast.single,
            fast.program_count
        ),
        (10_000_000, 1, NONE, 1)
    );
    assert_eq!(image.task_programs(fast).collect::<Vec<_>>(), [body(0)]);
    let background = tasks
        .iter()
        .find(|task| task.priority == 0xFFFF_FFFF)
        .expect("the record of the programs without a task");
    assert_eq!(background.program_count, 1);
    assert_eq!(
        image.task_programs(background).collect::<Vec<_>>(),
        [body(1)]
    );

    let bindings: Vec<IoBindRecord> = image
        .table::<IoBindRecord>()
        .expect("the bindings")
        .iter()
        .collect();
    assert_eq!(bindings.len(), 1);
    let (start, _) = image
        .find_symbol("MAIN.START")
        .expect("Main.Start's symbol");
    assert_eq!(bindings[0].symbol, start);
    assert_eq!(
        image.binding_address(&bindings[0]),
        Some(IoAddress::parse("%IX0.1").expect("an address"))
    );

    let statements: Vec<StmtRecord> = image
        .table::<StmtRecord>()
        .expect("the statement table")
        .iter()
        .collect();
    let places: Vec<(u16, u32, u16, u32)> = statements
        .iter()
        .map(|statement| {
            (
                statement.file,
                statement.start_line,
                statement.start_column,
                statement.end_line,
            )
        })
        .collect();
    assert_eq!(places, [(0, 7, 1, 9), (0, 8, 5, 8), (0, 16, 1, 16)]);
    assert_eq!(statements[0].pou, statements[1].pou);
    assert_ne!(statements[1].pou, statements[2].pou);

    let files: Vec<FileRecord> = image
        .table::<FileRecord>()
        .expect("the files")
        .iter()
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(image.string(files[0].label), Some("src/probe.st"));
}
