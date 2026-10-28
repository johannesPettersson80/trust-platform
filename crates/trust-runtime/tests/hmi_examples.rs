//! Specification 39, §17.2: the shipped examples' screens, rebuilt with the HMI Builder, pass the
//! deployment checks against each example's own program and write policy (§16.4).

use std::path::Path;
use std::sync::Arc;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::hmi::access::config::hmi_access_from_runtime_toml;
use trust_runtime::hmi::bundle::checks::{check_bundle, Program};
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::exchange::HmiExchange;
use trust_runtime::value::Duration;

const EXAMPLES: &[&str] = &[
    "hmi_tank_level",
    "hmi_equipment",
    "room_heating",
    "plant_demo",
    "web_ui_complete_project",
    "hardware_8do",
    "memory_marker_counter",
    "network_canvas_demo",
    "communication/ads_server_basic",
    "tutorials/12_hmi_pid_process_dashboard",
];

/// The program as `trust-runtime build` compiles it, without runtime-ignored sources.
fn program_sources(root: &Path, example: &str) -> Vec<SourceFile> {
    collect_project_source_files(root, None)
        .unwrap_or_else(|error| panic!("{example}: {error}"))
        .into_iter()
        .filter(|source| {
            !source
                .text
                .lines()
                .take(20)
                .any(|line| line.contains("@trustlsp:runtime-ignore"))
        })
        .collect()
}

/// Specification 39, §18 Phase 5: every example with screens calls `Hmi.Display` in a task, so the
/// runtime serves its screens.
#[test]
fn every_example_with_screens_calls_hmi_display() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut failures = Vec::new();
    for example in EXAMPLES {
        let root = repo.join("examples").join(example);
        let mut runtime = TestHarness::from_session(CompileSession::from_sources(program_sources(
            &root, example,
        )))
        .unwrap_or_else(|error| panic!("{example} compiles: {error}"))
        .into_runtime();
        let exchange = Arc::new(HmiExchange::new());
        runtime.set_hmi_exchange(Arc::clone(&exchange));
        for _ in 0..200 {
            let now = runtime.current_time().as_nanos() + Duration::from_millis(10).as_nanos();
            runtime.set_current_time(Duration::from_nanos(now));
            if let Err(error) = runtime.execute_cycle() {
                failures.push(format!("{example}: the cycle fails: {error}"));
                break;
            }
        }
        if exchange.calls() == 0 {
            failures.push(format!("{example}: no task calls Hmi.Display"));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn every_example_screen_passes_the_deployment_checks_against_its_program() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut failures = Vec::new();
    for example in EXAMPLES {
        let root = repo.join("examples").join(example);
        let bundle = HmiBundle::read(&root).unwrap_or_default();
        let pages = bundle
            .files
            .keys()
            .filter(|file| file.starts_with("hmi/pages/"))
            .count();
        if !bundle.files.contains_key("hmi/screen.toml") || pages == 0 {
            failures.push(format!("{example}: no screen layout or page"));
            continue;
        }
        let declarations = CompileSession::from_sources(program_sources(&root, example))
            .declarations()
            .unwrap_or_else(|error| panic!("{example} compiles: {error}"));
        let policy = std::fs::read_to_string(root.join("runtime.toml"))
            .ok()
            .map(|text| {
                hmi_access_from_runtime_toml(&text)
                    .unwrap_or_else(|error| panic!("{example}: {error}"))
                    .write
            })
            .unwrap_or_default();
        for problem in check_bundle(
            &bundle,
            Catalogue::shipped(),
            &Program {
                metadata: declarations.metadata(),
                storage: declarations.storage(),
            },
            &policy,
        ) {
            failures.push(format!(
                "{example}: {}:{}: {}",
                problem.file, problem.line, problem.message
            ));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
}

/// Specification 39, §14.3 and §17.2: once the symbols were approved (2A), the examples show
/// their pumps and valves with them, bound to the equipment's states, and no status pill or
/// gauge stands in for them any more.
#[test]
fn the_examples_show_their_pumps_and_valves_with_the_symbols() {
    type Symbol = (
        &'static str,
        &'static str,
        &'static [(&'static str, &'static str)],
    );
    let expected: &[(&str, &[Symbol], &[&str])] = &[
        (
            "hmi_tank_level",
            &[
                (
                    "pump",
                    "P-101",
                    &[
                        ("running", "Tank.Pump.Running"),
                        ("fault", "Tank.Pump.Tripped"),
                    ],
                ),
                (
                    "control-valve",
                    "FV-101",
                    &[("position", "Tank.ValveOpening")],
                ),
            ],
            &["Pump P-101", "Valve FV-101"],
        ),
        (
            "tutorials/12_hmi_pid_process_dashboard",
            &[
                ("pump", "P-1", &[("running", "P1.PumpRunning")]),
                ("control-valve", "FV-1", &[("position", "P1.ValvePosition")]),
                ("valve", "XV-2", &[("open", "P1.BypassOpen")]),
            ],
            &["Pump", "Valve", "Valve open", "Bypass open"],
        ),
        (
            "plant_demo",
            &[("pump", "P1", &[("running", "P1.Status.Running")])],
            &[],
        ),
    ];
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (example, symbols, replaced) in expected {
        let file = repo
            .join("examples")
            .join(example)
            .join("hmi/pages/overview.toml");
        let text =
            std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{example}: {error}"));
        let page = trust_runtime::hmi::page_files::parse_page("hmi/pages/overview.toml", &text)
            .unwrap_or_else(|errors| panic!("{example}: {errors:?}"));
        for (kind, tag, bindings) in *symbols {
            let object = page
                .objects
                .iter()
                .find(|object| object.kind == *kind && object.tag.as_deref() == Some(*tag))
                .unwrap_or_else(|| panic!("{example}: no {kind} {tag}"));
            for (property, path) in *bindings {
                assert_eq!(
                    object
                        .bind
                        .get(*property)
                        .map(|binding| binding.path.as_str()),
                    Some(*path),
                    "{example}: {kind} {tag} binds {property}"
                );
            }
        }
        for label in *replaced {
            assert!(
                !page.objects.iter().any(|object| {
                    matches!(object.kind.as_str(), "status" | "gauge")
                        && object.label.as_deref() == Some(*label)
                }),
                "{example}: the {label} stand-in is gone"
            );
        }
    }
}
