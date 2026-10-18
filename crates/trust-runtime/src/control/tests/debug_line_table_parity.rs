// Owner decision Q4 (18 October 2026): trust-runtime sets every breakpoint, step and stop location
// from the installed module's `LINE_TABLE`, and no answer changes (specification 13, "One resolver
// on every runtime" and "Breakpoint evaluation, source resolution, and hook contract";
// specification 12 §6.13 and §10). Each program runs through the control protocol twice: as built,
// with its `LINE_TABLE`, and with that section stripped. The stripped run keeps today's
// statement-index path, because a module without the section executes exactly as one with it
// (specification 12 §6.13). The set-breakpoints replies, the stop events, the stacks, the
// breakpoint-location listings and the step sequences of the two runs must agree answer for answer.

/// The path every program is registered under, relative to the project root.
const PARITY_PATH: &str = "src/main.st";

/// The program whose control state the parity runs replace: it supplies only the fields the
/// parity runs do not set.
const PARITY_BASE_SOURCE: &str = "PROGRAM Base\nVAR\nx : INT;\nEND_VAR\nx := 1;\nEND_PROGRAM\n";

/// Holds that one scan may take before the parity run gives up, so a stuck hold fails the test.
const PARITY_MAX_HOLDS: usize = 64;

/// The emission test's program (mirrors `SOURCE` in `line_table_emission.rs`), with a
/// configuration below its last line so that the scan runs `Main`. Line 17 is the FOR body, line
/// 19 the call and line 5 the statement inside `Add`.
const PARITY_EMISSION_SOURCE: &str = concat!(
    "FUNCTION Add : INT\n",
    "VAR_INPUT\n",
    "x : INT;\n",
    "END_VAR\n",
    "Add := x + INT#1;\n",
    "END_FUNCTION\n",
    "PROGRAM Main\n",
    "VAR\n",
    "f : BOOL;\n",
    "i : INT;\n",
    "END_VAR\n",
    "f := TRUE;\n",
    "IF f THEN\n",
    "i := 1;\n",
    "END_IF;\n",
    "FOR i := 0 TO 1 DO\n",
    "f := FALSE;\n",
    "END_FOR;\n",
    "i := Add(x := INT#1);\n",
    "END_PROGRAM\n",
    "CONFIGURATION Emission\n",
    "PROGRAM P1 : Main;\n",
    "END_CONFIGURATION\n",
);

/// The emission test's nested program (mirrors `NESTED_SOURCE` in `line_table_emission.rs`): a
/// labelled statement and a REPEAT whose statements share starts, with a configuration appended.
const PARITY_NESTED_SOURCE: &str = concat!(
    "PROGRAM Main\n",
    "VAR\n",
    "i : INT;\n",
    "f : BOOL;\n",
    "END_VAR\n",
    "REPEAT\n",
    "i := i + INT#1;\n",
    "UNTIL i > INT#3\n",
    "END_REPEAT;\n",
    "Top: f := TRUE;\n",
    "END_PROGRAM\n",
    "CONFIGURATION Nested\n",
    "PROGRAM P1 : Main;\n",
    "END_CONFIGURATION\n",
);

/// Every control answer of one run, in request order, under a label that names the request.
type ParityTranscript = Vec<(String, serde_json::Value)>;

/// What one program's run recorded: its answers, its runtime metadata and how often a scan held.
struct ParityProgramRun {
    transcript: ParityTranscript,
    metadata: crate::RuntimeMetadata,
    file_id: u32,
    holds: usize,
}

/// The bounded-C corpus programs, each `src/*.st` under `tests/fixtures/bounded-c`, in name order.
fn parity_corpus() -> Vec<(String, String)> {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bounded-c");
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(&root).expect("bounded-c corpus directory") {
        let source_dir = entry.expect("corpus entry").path().join("src");
        if !source_dir.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&source_dir).expect("corpus source directory") {
            let path = file.expect("corpus source entry").path();
            if path.extension().is_some_and(|extension| extension == "st") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    sources
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("corpus program");
            (path.display().to_string(), text)
        })
        .collect()
}

/// The control state and runtime for one build of `text`. With `stripped`, the runtime installs
/// the same artifact without its `LINE_TABLE`, against the same statement index.
fn parity_build(
    text: &str,
    stripped: bool,
) -> (ControlState, crate::Runtime, u32, crate::RuntimeMetadata) {
    let session =
        crate::harness::CompileSession::from_sources(vec![crate::harness::SourceFile::with_path(
            PARITY_PATH,
            text,
        )]);
    let built = session.build_runtime().expect("the program builds");
    let file_id = built
        .metadata_snapshot()
        .source_file_id(PARITY_PATH)
        .expect("the program's file has an id");
    let mut runtime = if stripped {
        let mut module = built
            .deployment_artifact()
            .expect("the installed artifact")
            .clone();
        module
            .sections
            .retain(|section| !matches!(section.data, crate::bytecode::SectionData::LineTable(_)));
        crate::Runtime::from_module_with_metadata(
            module,
            Some(std::sync::Arc::new(built.metadata_snapshot())),
            0,
            trust_runtime_core::vm::PreparationLimits::default(),
        )
        .expect("the artifact without LINE_TABLE installs")
    } else {
        built
    };
    let debug = runtime.enable_debug();
    let metadata = runtime.metadata_snapshot();
    let mut state = hmi_test_state(PARITY_BASE_SOURCE);
    let root = std::env::temp_dir().join("trust-line-table-parity");
    state.project_root = Some(root.clone());
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: file_id,
        path: root.join("src").join("main.st"),
        text: text.to_string(),
    }]);
    state.debug = debug;
    state.metadata = std::sync::Arc::new(std::sync::Mutex::new(metadata.clone()));
    (state, runtime, file_id, metadata)
}

/// Send one control request and record its whole answer under `label`.
fn parity_request(
    state: &ControlState,
    transcript: &mut ParityTranscript,
    label: String,
    body: serde_json::Value,
) {
    let response = handle_request_value(body, state, None);
    let answer = serde_json::json!({
        "ok": response.ok,
        "result": response.result,
        "error": response.error,
    });
    transcript.push((label, answer));
}

/// Send a breakpoint-location listing and record its answer. The listing names a set of positions
/// (specification 13, "Breakpoint Locations"): a label and the statement it labels share one code
/// offset, so the two runs may list them in either order. The positions are recorded in
/// (line, column) order, which keeps every position and every repeat in the comparison.
fn parity_listing(
    state: &ControlState,
    transcript: &mut ParityTranscript,
    label: String,
    body: serde_json::Value,
) {
    let response = handle_request_value(body, state, None);
    let mut result = response.result;
    if let Some(positions) = result
        .as_mut()
        .and_then(|result| result.get_mut("breakpoints"))
        .and_then(serde_json::Value::as_array_mut)
    {
        positions.sort_by_key(|position| (position["line"].as_u64(), position["column"].as_u64()));
    }
    let answer = serde_json::json!({
        "ok": response.ok,
        "result": result,
        "error": response.error,
    });
    transcript.push((label, answer));
}

/// The breakpoint-location listings of one line: the whole line, then every column either filter
/// can select.
fn parity_listings(
    state: &ControlState,
    transcript: &mut ParityTranscript,
    line: u32,
    length: u32,
) {
    parity_listing(
        state,
        transcript,
        format!("line {line} listing"),
        serde_json::json!({"id": 6, "type": "debug.breakpoint_locations",
            "params": {"source": PARITY_PATH, "line": line}}),
    );
    for column in 0..=length + 2 {
        parity_listing(
            state,
            transcript,
            format!("line {line} listing from column {column}"),
            serde_json::json!({"id": 6, "type": "debug.breakpoint_locations",
                "params": {"source": PARITY_PATH, "line": line, "column": column}}),
        );
        parity_listing(
            state,
            transcript,
            format!("line {line} listing to column {column}"),
            serde_json::json!({"id": 6, "type": "debug.breakpoint_locations",
                "params": {"source": PARITY_PATH, "line": line, "end_column": column}}),
        );
    }
}

/// Set one line's breakpoint through the control protocol.
fn parity_set_breakpoint(
    state: &ControlState,
    transcript: &mut ParityTranscript,
    label: String,
    line: u32,
) {
    parity_request(
        state,
        transcript,
        label,
        serde_json::json!({"id": 1, "type": "breakpoints.set",
            "params": {"source": PARITY_PATH, "lines": [line]}}),
    );
}

/// Run one scan on a worker thread, which returns its runtime when the scan ends. The simulator
/// clock first moves a second on, so that every task of the corpus, whose intervals are shorter,
/// is due in this scan; the clock is logical, so both runs of a program see the same times.
fn parity_start_scan(
    mut runtime: crate::Runtime,
) -> std::sync::mpsc::Receiver<(crate::Runtime, Result<(), String>)> {
    runtime.advance_time(crate::value::Duration::from_millis(1000));
    let (sender, ended) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut runtime = runtime;
        let result = runtime.execute_cycle().map_err(|error| error.to_string());
        // A closed receiver means the test has already failed; the runtime is dropped with it.
        let _ = sender.send((runtime, result));
    });
    ended
}

/// Where a scan stands: held at a debugger stop, or ended with its runtime and result.
enum ParityScan {
    Held,
    Ended(Box<crate::Runtime>, Result<(), String>),
}

/// Wait until the scan holds at a debugger stop or ends. A scan that does neither within the
/// deadline fails the test rather than wedging it.
fn parity_settle(
    ended: &std::sync::mpsc::Receiver<(crate::Runtime, Result<(), String>)>,
    debug: &crate::debug::DebugControl,
    label: &str,
) -> ParityScan {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match ended.try_recv() {
            Ok((runtime, result)) => return ParityScan::Ended(Box::new(runtime), result),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("{label}: the scan thread ended without its runtime")
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if debug.is_paused() {
            return ParityScan::Held;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{label}: the scan neither held at a stop nor ended"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Drive one scan to its end. At each debugger hold the run records what the protocol reports
/// (the stops and the stack), then answers with the next of `answers`, or resumes. Returns the
/// runtime and the number of holds.
fn parity_drive_scan(
    state: &ControlState,
    runtime: crate::Runtime,
    answers: &[&str],
    label: &str,
    transcript: &mut ParityTranscript,
) -> (crate::Runtime, usize) {
    let debug = state.debug.clone();
    let ended = parity_start_scan(runtime);
    let mut answers = answers.iter().copied();
    let mut holds = 0;
    loop {
        match parity_settle(&ended, &debug, label) {
            ParityScan::Ended(runtime, result) => {
                let outcome = match result {
                    Ok(()) => serde_json::json!({"ended": true}),
                    Err(error) => serde_json::json!({"ended": true, "error": error}),
                };
                transcript.push((format!("{label}: end of scan"), outcome));
                return (*runtime, holds);
            }
            ParityScan::Held => {
                holds += 1;
                assert!(
                    holds <= PARITY_MAX_HOLDS,
                    "{label}: the scan held more than {PARITY_MAX_HOLDS} times"
                );
                parity_request(
                    state,
                    transcript,
                    format!("{label}: stops"),
                    serde_json::json!({"id": 3, "type": "debug.stops"}),
                );
                parity_request(
                    state,
                    transcript,
                    format!("{label}: stack"),
                    serde_json::json!({"id": 4, "type": "debug.stack"}),
                );
                let answer = answers.next().unwrap_or("resume");
                parity_request(
                    state,
                    transcript,
                    format!("{label}: {answer}"),
                    serde_json::json!({"id": 5, "type": answer}),
                );
            }
        }
    }
}

/// The step sequences to run after the line sweep: from the first line that stopped, each step
/// kind, and for the emission program the FOR body, the call and the statement inside `Add`.
fn parity_scenarios(name: &str, first_stop: Option<u32>) -> Vec<(String, u32, Vec<&'static str>)> {
    let mut scenarios = Vec::new();
    if let Some(line) = first_stop {
        scenarios.push(("first over".to_string(), line, vec!["step_over"; 8]));
        scenarios.push(("first in".to_string(), line, vec!["step_in"; 8]));
        scenarios.push(("first out".to_string(), line, vec!["step_out"; 8]));
    }
    if name == "emission SOURCE" {
        scenarios.push(("FOR body over".to_string(), 16, vec!["step_over"; 8]));
        scenarios.push((
            "call in".to_string(),
            18,
            vec!["step_in", "step_in", "step_in", "step_out", "step_out"],
        ));
        scenarios.push(("callee out".to_string(), 4, vec!["step_out"]));
    }
    scenarios
}

/// Run one program through the protocol: a breakpoint on each line in turn with its scan, the
/// listings of each line, then the step scenarios on the same runtime.
fn parity_run(name: &str, text: &str, stripped: bool) -> ParityProgramRun {
    let (state, mut runtime, file_id, metadata) = parity_build(text, stripped);
    let mut transcript = Vec::new();
    let mut holds = 0;
    let mut first_stop = None;
    for (line, line_text) in text.split('\n').enumerate() {
        let line = u32::try_from(line).expect("a line number fits u32");
        let length = u32::try_from(line_text.len()).expect("a line length fits u32");
        parity_set_breakpoint(
            &state,
            &mut transcript,
            format!("line {line} breakpoints.set"),
            line,
        );
        let (back, stopped) = parity_drive_scan(
            &state,
            runtime,
            &[],
            &format!("line {line}"),
            &mut transcript,
        );
        runtime = back;
        if stopped > 0 && first_stop.is_none() {
            first_stop = Some(line);
        }
        holds += stopped;
        parity_listings(&state, &mut transcript, line, length);
    }
    for (scenario, line, answers) in parity_scenarios(name, first_stop) {
        parity_set_breakpoint(
            &state,
            &mut transcript,
            format!("{scenario}: breakpoints.set line {line}"),
            line,
        );
        let (back, stopped) = parity_drive_scan(
            &state,
            runtime,
            &answers,
            &format!("{scenario} from line {line}"),
            &mut transcript,
        );
        runtime = back;
        holds += stopped;
    }
    // A program whose every scan is refused before it runs a statement cannot stop anywhere. Its
    // refusal is part of the transcript, so both runs must refuse alike, which the comparison
    // proves. Any other program must stop, or the parity would be vacuous.
    let refused = transcript
        .iter()
        .filter(|(label, _)| label.ends_with("end of scan"))
        .all(|(_, answer)| answer.get("error").is_some_and(|error| !error.is_null()));
    if holds == 0 && refused {
        let first_error = transcript
            .iter()
            .find_map(|(_, answer)| answer.get("error").filter(|error| !error.is_null()));
        println!("line-table parity: {name} (stripped: {stripped}): no scan ran a statement, refused alike: {first_error:?}");
    }
    assert!(
        holds > 0 || refused,
        "{name} (stripped: {stripped}): no breakpoint stopped a scan that ran"
    );
    ParityProgramRun {
        transcript,
        metadata,
        file_id,
        holds,
    }
}

/// Every answer of the table run equals the statement-index run's, in the same order.
fn parity_assert_transcripts_agree(
    name: &str,
    table: &ParityTranscript,
    stripped: &ParityTranscript,
) {
    assert_eq!(
        table.len(),
        stripped.len(),
        "{name}: the two runs answered a different number of requests"
    );
    for ((label, ours), (other, theirs)) in table.iter().zip(stripped) {
        assert_eq!(
            label, other,
            "{name}: the two runs diverge in request order"
        );
        assert_eq!(
            ours, theirs,
            "{name}, {label}: the line table answer differs from the statement-index answer"
        );
    }
}

/// The statement spans of a file, sorted, so that two listings compare as sets (specification 13,
/// "Breakpoint Locations", names a set of positions).
fn parity_sorted_locations(
    metadata: &crate::RuntimeMetadata,
    file_id: u32,
) -> Vec<crate::debug::SourceLocation> {
    let mut locations = metadata
        .statement_locations(file_id)
        .map(<[crate::debug::SourceLocation]>::to_vec)
        .unwrap_or_default();
    locations.sort_by_key(|location| (location.start, location.end));
    locations
}

/// The resolver answers of both runs agree for every line and every column, through the
/// installation's metadata. The protocol sends only column 1 to set a breakpoint, so this covers
/// every column the control layer can send on that path (column 2 in the table's 1-based terms).
/// Returns the number of (line, column) cases compared.
fn parity_assert_resolvers_agree(
    name: &str,
    text: &str,
    table: &ParityProgramRun,
    stripped: &ParityProgramRun,
) -> usize {
    let mut cases = 0usize;
    assert!(
        table.metadata.line_table().is_some(),
        "{name}: the table run binds its LINE_TABLE"
    );
    assert!(
        stripped.metadata.line_table().is_none(),
        "{name}: the stripped run keeps the statement index"
    );
    assert_eq!(table.file_id, stripped.file_id, "{name}: the file id");
    let file_id = table.file_id;
    assert_eq!(
        parity_sorted_locations(&table.metadata, file_id),
        parity_sorted_locations(&stripped.metadata, file_id),
        "{name}: the breakpoint-location listing"
    );
    for (line, line_text) in text.split('\n').enumerate() {
        let line = u32::try_from(line).expect("a line number fits u32");
        let length = u32::try_from(line_text.len()).expect("a line length fits u32");
        for column in 0..=length + 2 {
            cases += 1;
            assert_eq!(
                table
                    .metadata
                    .resolve_breakpoint_position(text, file_id, line, column),
                stripped
                    .metadata
                    .resolve_breakpoint_position(text, file_id, line, column),
                "{name}, line {line}, column {column}: resolve_breakpoint_position"
            );
            assert_eq!(
                table
                    .metadata
                    .resolve_breakpoint_location(text, file_id, line, column),
                stripped
                    .metadata
                    .resolve_breakpoint_location(text, file_id, line, column),
                "{name}, line {line}, column {column}: resolve_breakpoint_location"
            );
        }
    }
    cases
}

#[test]
fn debug_line_table_parity_through_the_control_protocol() {
    let mut programs = vec![
        (
            "emission SOURCE".to_string(),
            PARITY_EMISSION_SOURCE.to_string(),
        ),
        (
            "emission NESTED_SOURCE".to_string(),
            PARITY_NESTED_SOURCE.to_string(),
        ),
    ];
    programs.extend(parity_corpus());
    let mut answers = 0usize;
    let mut stops = 0usize;
    let mut columns = 0usize;
    for (name, text) in &programs {
        let table = parity_run(name, text, false);
        let stripped = parity_run(name, text, true);
        parity_assert_transcripts_agree(name, &table.transcript, &stripped.transcript);
        columns += parity_assert_resolvers_agree(name, text, &table, &stripped);
        answers += table.transcript.len();
        stops += table.holds;
    }
    println!(
        "line-table parity: {} programs, {answers} control answers, {stops} stops, {columns} (line, column) resolver cases compared",
        programs.len()
    );
}
