//! The debugger's view of a compiled program (specification 44 §5.7, §8.2, §13.2): a debug image
//! names its POUs and their frame variables in its symbol table, links a frame into the context on
//! every POU entry, and stops at an armed statement with the call stack and the frames' variables
//! readable and writable; a release image has none of this and never stops.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build, load, Device, HostCode, NativeProgram, Options, ScanHooks, Target, Toolchain,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::image::{Image, StmtRecord, SymbolRecord, NONE};
use trust_runtime_core::native::memory::{
    pou_symbol, read_place, symbol_place, write_place, Place, ProgramMemory, StopFrame,
};
use trust_runtime_core::native::stop::Stop;

/// `Main` calls the instance `c` of `Counter`, whose body calls the function `Twice`. In the first
/// scan `count` is 0, so `Twice(1)` doubles 1 into 2, `count` is 2 and `total` 2.
const SOURCE: &str = "
FUNCTION Twice : INT
VAR_INPUT
    n : INT;
END_VAR
VAR
    doubled : INT;
END_VAR
doubled := n * 2;
Twice := doubled;
END_FUNCTION

FUNCTION_BLOCK Counter
VAR_OUTPUT
    count : INT;
END_VAR
count := Twice(count + 1);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    total : INT;
END_VAR
c();
total := c.count;
END_PROGRAM
";

fn program(hooks: bool, name: &str) -> NativeProgram<'static, HostCode> {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-{name}-{}", std::process::id()));
    let built = build(
        &[SourceFile::new(SOURCE)],
        &Options {
            target: host,
            hooks,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
    };
    let image: &'static [u8] = Box::leak(built.image.into_boxed_slice());
    let mut program = load(image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
}

/// The id of the statement that starts the source line that begins with `text`.
fn statement(image: &Image<'_>, text: &str) -> u32 {
    let line = SOURCE
        .lines()
        .position(|line| line.trim_start().starts_with(text))
        .expect("the line is in the source");
    let line = u32::try_from(line + 1).unwrap();
    let statements = image.table::<StmtRecord>().expect("the statement table");
    (0u32..)
        .zip(statements.iter())
        .find(|(_, record)| record.start_line == line)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no statement starts line {line}"))
}

/// The record of the symbol named `canonical`.
fn symbol(image: &Image<'_>, canonical: &str) -> (u32, SymbolRecord) {
    image
        .find_symbol(canonical)
        .unwrap_or_else(|| panic!("{canonical} has a symbol"))
}

/// What one stop saw.
#[derive(Debug)]
struct Seen {
    statement: u32,
    depth: u32,
    frames: Vec<StopFrame>,
    names: Vec<String>,
    n: Value,
    total: Value,
}

/// Hooks that record each stop, read `n` of the innermost frame and `MAIN.TOTAL`, and write
/// `n` := 5.
#[derive(Default)]
struct Recorder {
    seen: Vec<Seen>,
}

impl ScanHooks for Recorder {
    fn on_stop(&mut self, stop: &mut Stop<'_>) -> Result<(), RuntimeError> {
        let image = stop.image();
        let frames = stop.frames();
        let names = frames
            .iter()
            .map(|frame| {
                let (_, record) = pou_symbol(&image, frame.pou).expect("each POU has a symbol");
                image.string(record.display).unwrap_or("").to_string()
            })
            .collect();
        let (_, n) = symbol(&image, "@TWICE.N");
        let (place, type_index) = symbol_place(&image, symbol(&image, "MAIN.TOTAL").0)?;
        let total = read_place(&*stop, place, type_index)?;
        let n_place = Place::Frame {
            frame: 0,
            offset: n.offset,
        };
        let n_value = read_place(&*stop, n_place, n.type_index)?;
        write_place(stop, n_place, n.type_index, &Value::Int(5))?;
        self.seen.push(Seen {
            statement: stop.statement(),
            depth: stop.depth(),
            frames,
            names,
            n: n_value,
            total,
        });
        Ok(())
    }
}

/// Specification 44 §5.7: a debug image has a POU symbol `@<POU>` for each POU, area 0, its
/// type the instance's type (none for a function), and the frame variables of a function as
/// symbols `@<POU>.<VAR>` in area 7 under the POU symbol, with the POU's id. Catches a debug image
/// without them, a frame variable outside its frame, and a POU symbol a variable's lookup finds.
#[test]
fn a_debug_image_names_its_pous_and_their_frame_variables() {
    let program = program(true, "debugger-symbols");
    let image = program.image();
    let (twice_index, twice) = symbol(&image, "@TWICE");
    assert_eq!(twice.area, 0);
    assert_eq!(twice.type_index, NONE);
    assert!(twice.size >= 6, "the frame holds the result, n and doubled");
    for variable in ["@TWICE.N", "@TWICE.DOUBLED", "@TWICE.TWICE"] {
        let (_, record) = symbol(&image, variable);
        assert_eq!(record.area, 7, "{variable}");
        assert_eq!(record.parent, twice_index, "{variable}");
        assert_eq!(record.pou, twice.pou, "{variable}");
        assert!(record.offset + record.size <= twice.size, "{variable}");
    }
    let (_, counter) = symbol(&image, "@COUNTER");
    assert_eq!(counter.type_index, symbol(&image, "MAIN.C").1.type_index);
    let (_, main) = symbol(&image, "@MAIN");
    assert_eq!(main.type_index, symbol(&image, "MAIN").1.type_index);
    assert!(image.find_variable("TWICE").is_err());
}

/// Specification 44 §13.2, worked by hand for the first scan: the breakpoint on `doubled := n * 2`
/// stops once, at call depth 3, the frames innermost first `Twice` at the statement, `Counter`
/// at its call `count := Twice(count + 1)` and `Main` at `c();`; `Counter` runs on the instance
/// `MAIN.C` and `Main` on its program instance, `Twice` on none but with frame variables. The stop
/// reads `n` = 1 and `total` = 0, not yet written, and writes `n` := 5, so `doubled` is 10,
/// `count` 10 and `total` 10. Catches a stop at another statement, a depth or a frame missed, a
/// caller placed at the held statement, a frame variable read at the wrong offset, and a write at
/// a stop that the program does not see.
#[test]
fn a_breakpoint_holds_the_scan_with_its_call_stack_and_frame_variables() {
    let mut program = program(true, "debugger-stop");
    let image = program.image();
    let target = statement(&image, "doubled := n * 2;");
    let call = statement(&image, "count := Twice(count + 1);");
    let body = statement(&image, "c();");
    program.arm(false, &|id| id == target);
    let mut recorder = Recorder::default();
    program
        .scan(Duration::ZERO, &mut recorder)
        .expect("the scan runs");
    assert_eq!(recorder.seen.len(), 1, "{:?}", recorder.seen);
    let seen = &recorder.seen[0];
    assert_eq!(seen.statement, target);
    assert_eq!(seen.depth, 3);
    assert_eq!(seen.names, ["Twice", "Counter", "Main"]);
    let statements: Vec<u32> = seen.frames.iter().map(|frame| frame.stmt).collect();
    assert_eq!(statements, [target, call, body]);
    let (_, instance) = symbol(&program.image(), "MAIN.C");
    assert_eq!(
        seen.frames[1].instance,
        Some(Place::Area {
            area: instance.area,
            offset: instance.offset
        })
    );
    let (_, main) = symbol(&program.image(), "MAIN");
    assert_eq!(
        seen.frames[2].instance,
        Some(Place::Area {
            area: main.area,
            offset: main.offset
        })
    );
    assert_eq!(seen.frames[0].instance, None);
    assert!(seen.frames[0].locals);
    assert_eq!(seen.n, Value::Int(1));
    assert_eq!(seen.total, Value::Int(0));
    assert_eq!(program.value("MAIN.TOTAL"), Some(Value::Int(10)));
    assert_eq!(program.value("MAIN.C.COUNT"), Some(Value::Int(10)));
}

/// Specification 44 §13.2: a release image links no frames and has no statement hooks, so no
/// statement stops however the statements are armed, and it has no POU or frame variable symbol.
/// The scan computes as the debug image's would without a stop: `total` 2. Catches a release image
/// that stops or names its frames.
#[test]
fn a_release_image_never_stops_and_names_no_frames() {
    let mut program = program(false, "debugger-release");
    let image = program.image();
    assert!(image.header().is_release());
    assert!(image.find_symbol("@TWICE").is_none());
    assert!(image.find_symbol("@TWICE.N").is_none());
    program.arm(true, &|_| true);
    let mut recorder = Recorder::default();
    program
        .scan(Duration::ZERO, &mut recorder)
        .expect("the scan runs");
    assert!(recorder.seen.is_empty());
    assert_eq!(program.value("MAIN.TOTAL"), Some(Value::Int(2)));
}
