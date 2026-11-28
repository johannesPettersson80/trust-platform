//! The gallery starter `plcopen-motion-single-axis` (specification 25 §3.5): its program moves the
//! virtual axis once at start and then on each rising edge of its request, as its README's
//! walkthrough says. The real `trust-runtime` runs a private copy of the example, and the test
//! writes the request with `ctl io-write`, as Live Values' Write does (the simulated input keeps the
//! value written). The starter's copy of the PLCopen motion library is the shipped library.
//!
//! The test needs the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::path::Path;
use std::time::Duration;

use runtime_process::{repo_root, ExampleRuntime, Runtime, Snapshot};

/// The longest wait for the runtime to show a state.
const WAIT: Duration = Duration::from_secs(60);

/// The REAL at `address`, read from its printed form (`Real(25.0)` gives 25.0).
fn real(snapshot: &Snapshot, address: &str) -> f64 {
    let text = snapshot.text(address);
    text.strip_suffix(')')
        .and_then(|rest| rest.split_once('('))
        .and_then(|(_, inner)| inner.parse().ok())
        .unwrap_or_else(|| panic!("{address} is not a number: {text}"))
}

/// The moves (`%MW0`), the position (`%MD4`) and the error (`%MW2`) of `snapshot`.
fn axis(snapshot: &Snapshot) -> (i64, f64, i64) {
    (
        snapshot.int("%MW0"),
        real(snapshot, "%MD4"),
        snapshot.int("%MW2"),
    )
}

/// Writes the request `%IX0.0`, then waits until a snapshot holds it.
fn request(runtime: &Runtime, value: bool) {
    let text = if value { "true" } else { "false" };
    runtime.ctl_ok(&["io-write", "%IX0.0", text]);
    let expected = if value { "Bool(true)" } else { "Bool(false)" };
    runtime.wait_for_snapshot(WAIT, "the request as written", |snapshot| {
        snapshot.text("%IX0.0") == expected
    });
}

#[test]
fn the_starter_moves_its_axis_on_each_rising_edge_of_its_request() {
    // examples/plcopen-motion-single-axis/README.md, worked from its program: at start the step
    // chain powers the axis and moves it to 25, one move without an error. Each rising edge of the
    // request, through an R_TRIG, moves it to the other end, the target x(n) = 25 - x(n - 1): to 0
    // with 2 moves, and after FALSE and TRUE back to 25 with 3. A request held TRUE is no edge and
    // moves nothing. The virtual axis has no drive, so it reaches each target at once. Catches a
    // starter that moves on the request's level rather than its edge, one whose request does not
    // move the axis, and a README whose values the program does not give.
    let runtime = ExampleRuntime::new("plcopen-motion-single-axis").start();
    let first = runtime.wait_for_snapshot(WAIT, "the first move to 25", |snapshot| {
        snapshot.int("%MW0") == 1
    });
    assert_eq!(axis(&first), (1, 25.0, 0), "{}", runtime.diagnostics());
    assert_eq!(first.text("%QX0.0"), "Bool(true)", "the axis is powered");

    request(&runtime, true);
    let moved =
        runtime.wait_for_snapshot(WAIT, "the move to 0", |snapshot| snapshot.int("%MW0") == 2);
    assert_eq!(axis(&moved), (2, 0.0, 0));

    // A second TRUE leaves the request TRUE: no edge, so the axis stays, over 50 more scans.
    request(&runtime, true);
    let scan = moved.scan();
    let held = runtime.wait_for_snapshot(WAIT, "50 scans with the request held", |snapshot| {
        snapshot.scan() >= scan + 50
    });
    assert_eq!(axis(&held), (2, 0.0, 0));

    request(&runtime, false);
    request(&runtime, true);
    let back = runtime.wait_for_snapshot(WAIT, "the move back to 25", |snapshot| {
        snapshot.int("%MW0") == 3
    });
    assert_eq!(axis(&back), (3, 25.0, 0));
}

/// The names of the files in `dir`, sorted.
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| {
            entry
                .expect("a folder entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn the_starters_motion_library_is_the_shipped_library() {
    // The starter carries its own copy of libraries/plcopen_motion/single_axis_core, so that the
    // gallery copies a project that builds alone. Catches a fix of the library that the starter's
    // copy misses, as the virtual axis's power off would have been (G348).
    let root = repo_root();
    let library = root.join("libraries/plcopen_motion/single_axis_core/src");
    let copy = root.join("examples/plcopen-motion-single-axis/libraries/plcopen_motion/src");
    let names = file_names(&library);
    assert_eq!(
        names,
        file_names(&copy),
        "the copy holds the library's files"
    );
    for name in &names {
        let shipped = std::fs::read(library.join(name)).expect("the library's file");
        let copied = std::fs::read(copy.join(name)).expect("the copy's file");
        assert!(
            shipped == copied,
            "{name} differs between {} and {}",
            library.display(),
            copy.display()
        );
    }
}
