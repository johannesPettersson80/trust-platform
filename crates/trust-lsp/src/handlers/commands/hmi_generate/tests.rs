//! Specification 39, §10: Create PLC code writes one declaration and one call per new equipment
//! instance inside its regions, reuses instances declared by hand, is idempotent, and asks for a
//! control cycle only when the project has no task.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::super::{LoadedSource, ProjectInputs};
use super::*;

/// The Plant library and the libraries it uses, as sources.
fn library_sources() -> Vec<LoadedSource> {
    sources_of(&["alarm", "control", "plant"])
}

/// The sources of the named repository libraries.
fn sources_of(names: &[&str]) -> Vec<LoadedSource> {
    let libraries = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries");
    let mut sources = Vec::new();
    for library in names {
        let folder = libraries.join(library).join("src");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)
            .expect("the library folder")
            .map(|entry| entry.expect("an entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "st"))
            .collect();
        files.sort();
        for path in files {
            let text = std::fs::read_to_string(&path).expect("the library source");
            sources.push(LoadedSource { path, text });
        }
    }
    sources
}

/// A temporary project folder, unique per test and process, removed afterwards.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "trust-create-plc-code-{}-{unique}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("a project folder");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project folder with the pump definition, `src/Main.st` and, when given, `plant.toml`.
struct Fixture {
    root: Folder,
    main: String,
    configuration: Option<String>,
    design: Option<String>,
}

impl Fixture {
    fn new(main: &str) -> Self {
        let root = Folder::new();
        let pairs = root.path().join("hmi/pairs");
        std::fs::create_dir_all(&pairs).expect("the pairs folder");
        let pump = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../libraries/plant/hmi/pairs/plant-pump.toml");
        std::fs::copy(pump, pairs.join("plant-pump.toml")).expect("the pump definition");
        Self {
            root,
            main: main.to_string(),
            configuration: Some(
                "CONFIGURATION Plant\nRESOURCE Cpu ON PLC\nTASK Fast (INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM Main WITH Fast : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n"
                    .to_string(),
            ),
            design: None,
        }
    }

    fn inputs(&self) -> ProjectInputs {
        let root = self.root.path().to_path_buf();
        let mut sources = library_sources();
        sources.push(LoadedSource {
            path: root.join("src/Main.st"),
            text: self.main.clone(),
        });
        if let Some(configuration) = &self.configuration {
            sources.push(LoadedSource {
                path: root.join("src/Configuration.st"),
                text: configuration.clone(),
            });
        }
        let mut open = BTreeMap::new();
        if let Some(design) = &self.design {
            open.insert(root.join("plant.toml"), design.clone());
        }
        ProjectInputs {
            sources_root: root.join("src"),
            root,
            sources,
            open,
        }
    }

    fn create(&self, cycle_ms: Option<u32>) -> Value {
        self.create_showing("Main.P101", cycle_ms)
            .expect("PLC code is created")
    }

    fn create_showing(&self, instance: &str, cycle_ms: Option<u32>) -> Result<Value, String> {
        let shown = BTreeMap::from([(instance.to_string(), "Plant.FB_Pump".to_string())]);
        let request = Request {
            selection: HmiBindingsCommandArgs {
                open: BTreeMap::new(),
                root_uri: None,
                text_document: None,
            },
            screen_task: None,
            cycle_ms,
        };
        create_from(&self.inputs(), &shown, &request)
    }
}

/// The new text of `src/Main.st` in an answer.
fn new_main(answer: &Value) -> Option<String> {
    answer["edit"]["changes"]
        .as_object()?
        .iter()
        .find(|(uri, _)| uri.ends_with("/src/Main.st"))
        .and_then(|(_, edits)| edits[0]["newText"].as_str())
        .map(str::to_string)
}

const MAIN: &str = "USING Plant;\n\nPROGRAM Main\nVAR\n    PumpRun AT %QX0.0 : BOOL;\nEND_VAR\n    ;\nEND_PROGRAM\n";

const DESIGN: &str = "format = 2\n\n[equipment.\"Main.P101\"]\nblock = \"Plant.FB_Pump\"\n\n[equipment.\"Main.P101\".settings]\nStartTimeout = \"T#10s\"\nStopTimeout = \"T#5s\"\nHasFeedback = \"FALSE\"\n\n[equipment.\"Main.P101\".signals]\nRunCommand = \"Main.PumpRun\"\n";

#[test]
fn a_shown_pump_gets_one_declaration_and_one_call_and_nothing_else() {
    let mut fixture = Fixture::new(MAIN);
    fixture.design = Some(DESIGN.to_string());
    let answer = fixture.create(None);
    assert_eq!(answer["ok"], true, "{answer}");
    let text = new_main(&answer).expect("src/Main.st changes");
    assert!(
        text.contains(
            "    P101 : Plant.FB_Pump := (HasFeedback := FALSE, StartTimeout := T#10s);\n"
        ),
        "the declaration holds only the values that differ from the block's defaults:\n{text}"
    );
    assert!(
        text.contains("    P101(RunCommand => PumpRun);\n"),
        "{text}"
    );
    assert_eq!(
        text.matches("P101").count(),
        2,
        "one declaration and one call:\n{text}"
    );
    assert!(
        answer["notes"].as_array().expect("notes").is_empty(),
        "{answer}"
    );
    assert_eq!(answer["expected"].as_object().expect("expected").len(), 1);
}

#[test]
fn creating_the_code_again_changes_nothing() {
    let mut fixture = Fixture::new(MAIN);
    fixture.design = Some(DESIGN.to_string());
    let first = new_main(&fixture.create(None)).expect("the first run changes src/Main.st");
    fixture.main = first;
    let again = fixture.create(None);
    assert_eq!(again["ok"], true, "{again}");
    assert!(
        new_main(&again).is_none(),
        "the second run is idempotent: {again}"
    );
}

#[test]
fn an_instance_declared_by_hand_is_reused_never_redeclared() {
    let main = MAIN.replace(
        "    PumpRun AT %QX0.0 : BOOL;\n",
        "    PumpRun AT %QX0.0 : BOOL;\n    P101 : Plant.FB_Pump;\n",
    );
    let fixture = Fixture::new(&main);
    let answer = fixture.create(None);
    assert_eq!(answer["ok"], true, "{answer}");
    assert!(
        new_main(&answer).is_none(),
        "nothing is written for it: {answer}"
    );
}

#[test]
fn a_missing_connection_is_listed_and_no_address_is_invented() {
    let fixture = Fixture::new(MAIN);
    let answer = fixture.create(None);
    let text = new_main(&answer).expect("src/Main.st changes");
    assert!(text.contains("    P101();\n"), "{text}");
    let notes: Vec<&str> = answer["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(|note| note["message"].as_str())
        .collect();
    assert!(
        notes.contains(&"Run command is not connected to hardware"),
        "{notes:?}"
    );
    assert!(
        notes.contains(&"Running feedback is not connected to hardware"),
        "{notes:?}"
    );
}

#[test]
fn a_project_without_a_task_is_asked_for_its_control_cycle() {
    let mut fixture = Fixture::new(MAIN);
    fixture.configuration = None;
    let asked = fixture.create(None);
    assert_eq!(asked, json!({"ok": false, "needsControlCycle": true}));
    let answer = fixture.create(Some(50));
    let created = answer["creates"].as_array().expect("creates");
    let configuration = created
        .iter()
        .find(|file| {
            file["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/src/plant-configuration.st"))
        })
        .expect("the configuration is created");
    assert!(configuration["text"]
        .as_str()
        .expect("its text")
        .contains("TASK ControlTask (INTERVAL := T#50ms, PRIORITY := 1);"));
}

#[test]
fn a_changed_design_rewrites_the_declaration_of_a_generated_instance() {
    let mut fixture = Fixture::new(MAIN);
    fixture.design = Some(DESIGN.to_string());
    fixture.main = new_main(&fixture.create(None)).expect("the code is created");
    let changed = DESIGN.replace("StartTimeout = \"T#10s\"", "StartTimeout = \"T#20s\"");
    fixture.design = Some(changed.clone());
    let inputs = fixture.inputs();
    let index = Declarations::read(&inputs.sources);
    let design = PlantDesign::parse(&changed).expect("the design loads");
    let definitions =
        super::super::hmi_program_edits::project_pairs_edited(&inputs.root, &inputs.open);
    assert!(generated(&inputs, &index, "Main.P101"));
    let (_, before, after) = refresh_regions(&inputs, &index, &design, &definitions, "Main.P101")
        .expect("the regions are written again")
        .expect("the regions hold the instance");
    assert_eq!(before, fixture.main);
    assert!(after.contains("StartTimeout := T#20s"), "{after}");
}

#[test]
fn a_renamed_instance_keeps_its_regions_and_a_value_changed_by_hand_is_refused() {
    let mut fixture = Fixture::new(MAIN);
    fixture.design = Some(DESIGN.to_string());
    let generated = new_main(&fixture.create(None)).expect("the code is created");
    // Renaming the instance changes its name in PLC code and in the design (§10.4).
    fixture.main = generated.replace("P101", "FeedPump");
    fixture.design = Some(DESIGN.replace("Main.P101", "Main.FeedPump"));
    let renamed = fixture
        .create_showing("Main.FeedPump", None)
        .expect("a renamed instance is accepted");
    let text = new_main(&renamed).expect("the checksums are written again");
    assert!(
        text.contains(
            "    FeedPump : Plant.FB_Pump := (HasFeedback := FALSE, StartTimeout := T#10s);\n"
        ),
        "{text}"
    );
    assert!(!text.contains("P101"), "{text}");
    fixture.main = generated.replace("StartTimeout := T#10s", "StartTimeout := T#30s");
    fixture.design = Some(DESIGN.to_string());
    let refused = fixture
        .create_showing("Main.P101", None)
        .expect_err("a hand edit is refused");
    assert!(refused.starts_with("src/Main.st:"), "{refused}");
    assert!(
        refused.contains("declarations region was changed by hand"),
        "{refused}"
    );
}

#[test]
fn a_terminal_needed_only_with_feedback_the_block_leaves_off_is_not_asked_for() {
    // FB_VariableSpeedFan declares HasSpeedFeedback without a value, so FALSE: its speed feedback
    // is not missing, while its running feedback (HasFeedback := TRUE) still is.
    let fixture = Fixture::new(MAIN);
    let fan = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../libraries/plant/hmi/pairs/plant-variable-speed-fan.toml");
    std::fs::copy(
        fan,
        fixture
            .root
            .path()
            .join("hmi/pairs/plant-variable-speed-fan.toml"),
    )
    .expect("the fan definition");
    let shown = BTreeMap::from([(
        "Main.F101".to_string(),
        "Plant.FB_VariableSpeedFan".to_string(),
    )]);
    let request = Request {
        selection: HmiBindingsCommandArgs {
            open: BTreeMap::new(),
            root_uri: None,
            text_document: None,
        },
        screen_task: None,
        cycle_ms: None,
    };
    let answer = create_from(&fixture.inputs(), &shown, &request).expect("PLC code is created");
    let notes: Vec<&str> = answer["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(|note| note["message"].as_str())
        .collect();
    assert!(
        notes
            .iter()
            .any(|note| note.starts_with("Running feedback")),
        "{notes:?}"
    );
    assert!(
        !notes.iter().any(|note| note.starts_with("Speed feedback")),
        "{notes:?}"
    );
}

#[test]
fn connecting_a_feedback_the_block_leaves_off_switches_it_on() {
    // FB_ControlValve declares HasFeedback without a value; its connected position feedback means
    // the valve has one, so the declaration sets it, and a valve without that terminal stays as is.
    let mut fixture = Fixture::new(MAIN);
    let valve = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../libraries/plant/hmi/pairs/plant-control-valve.toml");
    std::fs::copy(
        valve,
        fixture
            .root
            .path()
            .join("hmi/pairs/plant-control-valve.toml"),
    )
    .expect("the valve definition");
    fixture.main = MAIN.replace(
        "    PumpRun AT %QX0.0 : BOOL;\n",
        "    PumpRun AT %QX0.0 : BOOL;\n    Demand AT %QL8 : LREAL;\n    Position AT %IL8 : LREAL;\n",
    );
    let request = Request {
        selection: HmiBindingsCommandArgs {
            open: BTreeMap::new(),
            root_uri: None,
            text_document: None,
        },
        screen_task: None,
        cycle_ms: None,
    };
    let shown = BTreeMap::from([
        (
            "Main.TV101".to_string(),
            "Plant.FB_ControlValve".to_string(),
        ),
        (
            "Main.TV102".to_string(),
            "Plant.FB_ControlValve".to_string(),
        ),
    ]);
    fixture.design = Some(
        "format = 2\n\n[equipment.\"Main.TV101\"]\nblock = \"Plant.FB_ControlValve\"\n\n[equipment.\"Main.TV101\".signals]\nPositionSetpoint = \"Main.Demand\"\nPositionFeedback = \"Main.Position\"\n"
            .to_string(),
    );
    let text = new_main(&create_from(&fixture.inputs(), &shown, &request).expect("created"))
        .expect("the program");
    assert!(
        text.contains("    TV101 : Plant.FB_ControlValve := (HasFeedback := TRUE);\n"),
        "{text}"
    );
    assert!(
        text.contains("    TV102 : Plant.FB_ControlValve;\n"),
        "{text}"
    );
}

/// A program measuring a tank temperature on a 4-20 mA input, for the PID loop tests.
const LOOP_MAIN: &str = "USING Plant;\n\nPROGRAM Main\nVAR\n    TT101_Signal : LREAL := 11.2;\nEND_VAR\n    ;\nEND_PROGRAM\n";

/// TIC101 holds TT101 with heater EH101, TIC102 drives valve TV101 from the same measurement.
const LOOP_DESIGN: &str = "format = 2\n\n[equipment.\"Main.TIC101\"]\nblock = \"Control.PID\"\n\n[equipment.\"Main.TIC101\".settings]\n\"{instance}_Parameters.ProportionalGain\" = \"2.0\"\n\n[equipment.\"Main.TIC101\".dependencies]\n\"{instance}_Input.ProcessValue\" = \"Main.TT101\"\n\"{instance}_Output\" = \"Main.EH101\"\n\n[equipment.\"Main.TIC102\"]\nblock = \"Control.PID\"\n\n[equipment.\"Main.TIC102\".dependencies]\n\"{instance}_Input.ProcessValue\" = \"Main.TT101\"\n\"{instance}_Output\" = \"Main.TV101\"\n\n[equipment.\"Main.TT101\"]\nblock = \"Plant.FB_TemperatureInput\"\n\n[equipment.\"Main.TT101\".signals]\nRaw = \"Main.TT101_Signal\"\n";

/// A fixture with the definitions of a PID loop, its measurement and the equipment it drives.
fn loop_fixture(main: &str, design: &str) -> Fixture {
    let mut fixture = Fixture::new(main);
    for pair in [
        "plant-pid.toml",
        "plant-heater.toml",
        "plant-control-valve.toml",
        "plant-temperature-input.toml",
    ] {
        let from = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../libraries/plant/hmi/pairs")
            .join(pair);
        std::fs::copy(from, fixture.root.path().join("hmi/pairs").join(pair))
            .expect("the definition");
    }
    fixture.design = Some(design.to_string());
    fixture
}

fn loop_shown(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, block)| (path.to_string(), block.to_string()))
        .collect()
}

fn plain_request() -> Request {
    Request {
        selection: HmiBindingsCommandArgs {
            open: BTreeMap::new(),
            root_uri: None,
            text_document: None,
        },
        screen_task: None,
        cycle_ms: None,
    }
}

#[test]
fn a_pid_loop_is_ordinary_code_around_control_pid() {
    // Specification 39, §10.5: no block around the PID; its variables beside it, those the
    // engineer and the operator set kept through a restart; its output drives a power or a
    // position by the interface the chosen instance implements.
    let fixture = loop_fixture(LOOP_MAIN, LOOP_DESIGN);
    let shown = loop_shown(&[
        ("Main.TT101", "Plant.FB_TemperatureInput"),
        ("Main.EH101", "Plant.FB_Heater"),
        ("Main.TV101", "Plant.FB_ControlValve"),
        ("Main.TIC101", "Control.PID"),
        ("Main.TIC102", "Control.PID"),
    ]);
    let answer = create_from(&fixture.inputs(), &shown, &plain_request()).expect("created");
    let text = new_main(&answer).expect("the program");
    for expected in [
        "USING Control;\n",
        "    TIC101 : Control.PID;\n",
        "    TIC101_Input : Control.PIDInput;\n",
        "    TIC101_Initialized : BOOL;\n",
        "VAR RETAIN\n    (* truST plant: retained declarations sha256:",
        "        ProportionalGain := 2.0,\n",
        "        AntiWindup := PIDAntiWindup#BackCalculation);\n",
        "    TIC101_Manual : BOOL := TRUE;\n",
        "    TIC101_Setpoint : LREAL;\n",
        "    TIC101.Configure(Parameters := TIC101_Parameters);\n",
        "    TIC101_Input.CycleTime := 0.1;\n",
        "    IF TT101.GetQuality() = E_Quality#Good THEN\n        TIC101_Input.ProcessValue := TT101.GetValue();\n",
        "    TIC101_Output := TIC101.Update(Input := TIC101_Input);\n",
        "    EH101.AutoPower := TIC101_Output;\n",
        "    TV101.AutoPosition := TIC102_Output;\n",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    assert!(!text.contains("EH101.AutoPosition"), "{text}");
    assert!(!text.contains("FB_PIDLoop"), "{text}");
    assert!(
        text.find("TT101(").expect("the measurement's call")
            < text.find("TIC101.Configure").expect("the loop's code"),
        "the measurement is called before the loop:\n{text}"
    );
    // The loop's own variables are not instances: written again, nothing changes.
    let mut again = fixture;
    again.main = text.clone();
    let second = create_from(&again.inputs(), &shown, &plain_request()).expect("created again");
    assert_eq!(new_main(&second), None, "{second}");
    // The operator runs the loop from its panel: its write rules, the setpoint within the
    // measurement's range (FB_AnalogInput's 0 to 100) and the output within its limits.
    let writes = answer["writes"].as_array().expect("the write rules");
    assert!(
        writes.contains(&serde_json::json!({"path": "Main.TIC101_Manual", "role": "operator"})),
        "{answer}"
    );
    assert!(writes.contains(&serde_json::json!({"path": "Main.TIC101_Setpoint", "role": "operator", "min": 0.0, "max": 100.0})), "{answer}");
    assert!(writes.contains(&serde_json::json!({"path": "Main.TIC101_ManualOutput", "role": "operator", "min": 0.0, "max": 100.0})), "{answer}");
}

#[test]
fn a_loop_whose_output_is_another_loops_setpoint_is_a_cascade() {
    // §10.5: the outer loop's output is the inner loop's setpoint; while the inner loop is in
    // Manual the outer one tracks its measurement, so closing the cascade is bumpless.
    let design = "format = 2\n\n[equipment.\"Main.TIC200\"]\nblock = \"Control.PID\"\n\n[equipment.\"Main.TIC200\".dependencies]\n\"{instance}_Input.ProcessValue\" = \"Main.TT101\"\n\"{instance}_Output\" = \"Main.TIC201\"\n\n[equipment.\"Main.TIC201\"]\nblock = \"Control.PID\"\n\n[equipment.\"Main.TIC201\".dependencies]\n\"{instance}_Input.ProcessValue\" = \"Main.TT102\"\n\"{instance}_Output\" = \"Main.EH101\"\n\n[equipment.\"Main.TT101\"]\nblock = \"Plant.FB_TemperatureInput\"\n\n[equipment.\"Main.TT102\"]\nblock = \"Plant.FB_TemperatureInput\"\n";
    let fixture = loop_fixture(MAIN, design);
    let shown = loop_shown(&[
        ("Main.TT101", "Plant.FB_TemperatureInput"),
        ("Main.TT102", "Plant.FB_TemperatureInput"),
        ("Main.EH101", "Plant.FB_Heater"),
        ("Main.TIC200", "Control.PID"),
        ("Main.TIC201", "Control.PID"),
    ]);
    let text =
        new_main(&create_from(&fixture.inputs(), &shown, &plain_request()).expect("created"))
            .expect("the program");
    for expected in [
        "    IF TIC201_Manual THEN\n",
        "        TIC200_Input.Mode := PIDMode#Track;\n",
        "        TIC200_Input.TrackingOutput := TIC201_Input.ProcessValue;\n",
        "    TIC201_Setpoint := TIC200_Output;\n",
        "    EH101.AutoPower := TIC201_Output;\n",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    assert!(
        text.find("TIC201_Setpoint := TIC200_Output")
            .expect("the cascade")
            < text.find("TIC201.Configure").expect("the inner loop"),
        "the outer loop runs first:\n{text}"
    );
}

#[test]
fn the_generated_loop_runs_bumpless_and_holds_on_a_bad_measurement() {
    // §10.5 executed: the code Create PLC code writes, compiled with the Plant, Control and Alarm
    // libraries and scanned on the VM at the task's 100 ms.
    use trust_runtime::harness::TestHarness;
    use trust_runtime::hmi::member_paths::{resolve_member, MemberPath};
    use trust_runtime::value::{Duration, Value as StValue};
    let fixture = loop_fixture(LOOP_MAIN, LOOP_DESIGN);
    let shown = loop_shown(&[
        ("Main.TT101", "Plant.FB_TemperatureInput"),
        ("Main.EH101", "Plant.FB_Heater"),
        ("Main.TIC101", "Control.PID"),
    ]);
    let main =
        new_main(&create_from(&fixture.inputs(), &shown, &plain_request()).expect("created"))
            .expect("the program");
    let mut sources: Vec<String> = library_sources()
        .into_iter()
        .map(|source| source.text)
        .collect();
    sources.push(main.clone());
    // The fixture's configuration is named `Plant`, as the Plant library's namespace is: compiled
    // together they would collide, so the program runs in a configuration of its own name.
    sources.push(
        "CONFIGURATION Cell\nRESOURCE Cpu ON PLC\nTASK Fast (INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM Main WITH Fast : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n"
            .to_string(),
    );
    let texts: Vec<&str> = sources.iter().map(String::as_str).collect();
    let mut harness = TestHarness::from_sources(&texts)
        .unwrap_or_else(|error| panic!("the generated program compiles: {error}\n{main}"));
    let scan = |harness: &mut TestHarness| {
        harness.advance_time(Duration::from_millis(100));
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    };
    let read = |harness: &TestHarness, path: &str| -> f64 {
        let parsed = MemberPath::parse(&format!("Main.{path}")).expect("a path");
        match resolve_member(&parsed, harness.runtime().storage()) {
            Some(StValue::LReal(value)) => *value,
            Some(StValue::Bool(flag)) => f64::from(u8::from(*flag)),
            other => panic!("{path} is {other:?}"),
        }
    };
    let set = |harness: &mut TestHarness, name: &str, value: StValue| {
        harness
            .try_set_input(name, value)
            .expect("a variable of Main");
    };
    scan(&mut harness);
    assert_eq!(
        read(&harness, "TIC101_Manual"),
        1.0,
        "a loop starts in Manual"
    );
    set(&mut harness, "TIC101_ManualOutput", StValue::LReal(35.0));
    set(&mut harness, "TIC101_Setpoint", StValue::LReal(50.0));
    scan(&mut harness);
    assert_eq!(
        read(&harness, "TIC101_Output"),
        35.0,
        "in Manual the output is the operator's"
    );
    // 11.2 mA is 45 °C: in Auto only the integral step Ki·dt·e acts, no jump of Kp·e.
    set(&mut harness, "TIC101_Manual", StValue::Bool(false));
    scan(&mut harness);
    let first = read(&harness, "TIC101_Output");
    assert!(
        (first - (35.0 + 0.1 * 0.1 * 5.0)).abs() < 1e-9,
        "bumpless into Auto: {first}"
    );
    assert_eq!(
        read(&harness, "TIC101_ManualOutput"),
        first,
        "in Auto the manual output follows"
    );
    for _ in 0..5 {
        scan(&mut harness);
    }
    let held = read(&harness, "TIC101_Output");
    assert!(held > first, "the loop integrates the deviation");
    // A broken wire: 2 mA is bad; the output holds in Manual.
    set(&mut harness, "TT101_Signal", StValue::LReal(2.0));
    for _ in 0..5 {
        scan(&mut harness);
    }
    assert_eq!(
        read(&harness, "TIC101_Output"),
        held,
        "a bad measurement holds the output"
    );
    // 12 mA is 50 °C: the loop starts again from the held output, without a jump of Kp·Δ.
    set(&mut harness, "TT101_Signal", StValue::LReal(12.0));
    scan(&mut harness);
    assert!(
        (read(&harness, "TIC101_Output") - held).abs() < 1e-6,
        "it resumes from the held output: {} after {held}",
        read(&harness, "TIC101_Output")
    );
    // A warm restart keeps what was set and the output; the loop starts again from it.
    harness
        .restart(trust_runtime::RestartMode::Warm)
        .expect("a warm restart");
    let kept = read(&harness, "TIC101_Output");
    assert_eq!(
        read(&harness, "TIC101_Setpoint"),
        50.0,
        "the setpoint is retained"
    );
    assert_eq!(read(&harness, "TIC101_Manual"), 0.0, "the mode is retained");
    scan(&mut harness);
    // The test program's input starts again at 11.2 mA (45 °C): initialised from that measurement
    // and the retained output, only the integral step Ki·dt·e acts, no jump of Kp·e.
    let resumed = read(&harness, "TIC101_Output");
    assert!(
        (resumed - (kept + 0.1 * 0.1 * 5.0)).abs() < 1e-9,
        "after the restart the loop continues from its output: {kept} -> {resumed}"
    );
    assert_eq!(
        read(&harness, "EH101.AutoPower"),
        read(&harness, "TIC101_Output")
    );
}

#[test]
fn a_rename_that_moves_a_call_is_accepted_and_a_changed_call_is_not() {
    // Calls of one kind are in name order, so renaming P102 to A102 moves its call first; the
    // region still holds only what the generator writes for its instances (§10.3 item 1).
    let mut fixture = Fixture::new(MAIN);
    let create = |fixture: &Fixture, names: &[&str]| {
        let shown: BTreeMap<String, String> = names
            .iter()
            .map(|name| (format!("Main.{name}"), "Plant.FB_Pump".to_string()))
            .collect();
        let request = Request {
            selection: HmiBindingsCommandArgs {
                open: BTreeMap::new(),
                root_uri: None,
                text_document: None,
            },
            screen_task: None,
            cycle_ms: None,
        };
        create_from(&fixture.inputs(), &shown, &request)
    };
    let calls = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| line.trim_end().ends_with("();"))
            .map(|line| line.trim().to_string())
            .collect()
    };
    let generated =
        new_main(&create(&fixture, &["P101", "P102"]).expect("created")).expect("the program");
    assert_eq!(calls(&generated), ["P101();", "P102();"], "{generated}");
    fixture.main = generated.replace("P102", "A102");
    let renamed =
        create(&fixture, &["P101", "A102"]).expect("a rename that moves a call is accepted");
    let text = new_main(&renamed).expect("the regions are written again");
    assert_eq!(calls(&text), ["A102();", "P101();"], "{text}");
    fixture.main = generated
        .replace("P102", "A102")
        .replace("    A102();", "    A102(AutoRun := TRUE);");
    let refused = create(&fixture, &["P101", "A102"]).expect_err("a changed call is refused");
    assert!(
        refused.contains("calls region was changed by hand"),
        "{refused}"
    );
}

#[test]
fn an_empty_screens_file_left_by_an_unsaved_run_is_written_again() {
    // An earlier version left `src/screens.st` empty when its text was never saved; the screens
    // program is written into it again, while a file with text of its own is refused.
    let mut fixture = Fixture::new(MAIN);
    fixture.design = Some(DESIGN.to_string());
    let screens = fixture.root.path().join("src/screens.st");
    std::fs::create_dir_all(screens.parent().expect("the source folder"))
        .expect("the source folder");
    std::fs::write(&screens, "").expect("an empty screens file");
    let mut inputs = fixture.inputs();
    inputs.sources.extend(sources_of(&["hmi"]));
    let shown = BTreeMap::from([("Main.P101".to_string(), "Plant.FB_Pump".to_string())]);
    let request = Request {
        selection: HmiBindingsCommandArgs {
            open: BTreeMap::new(),
            root_uri: None,
            text_document: None,
        },
        screen_task: None,
        cycle_ms: None,
    };
    let answer = create_from(&inputs, &shown, &request).expect("PLC code is created");
    let created: Vec<&str> = answer["creates"]
        .as_array()
        .expect("the created files")
        .iter()
        .filter_map(|file| file["uri"].as_str())
        .collect();
    assert!(
        created.iter().any(|uri| uri.ends_with("/src/screens.st")),
        "{answer}"
    );
    std::fs::write(&screens, "PROGRAM Mine\nEND_PROGRAM\n").expect("the user's own screens file");
    let refused = create_from(&inputs, &shown, &request);
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.contains("src/screens.st already exists")),
        "{refused:?}"
    );
}
