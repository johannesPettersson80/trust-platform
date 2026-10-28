use super::{
    canonicalize_lossy, collect_sources, CompileSession, HarnessSourceFile, SourceOptions,
};
use trust_runtime::harness::TestHarness;

#[test]
fn lesson1_simulator_source_selection_excludes_native_tests() {
    // Specification 38 §3.1: native tests are not running PLC instances.
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy");
    let entry = project.join("src/Main.st");
    let sources = collect_sources(
        entry.to_str().unwrap(),
        &SourceOptions {
            root: Some(project.to_string_lossy().into_owned()),
            include_globs: vec!["**/*.st".to_owned()],
            ..SourceOptions::default()
        },
    )
    .expect("discover the delivered lesson sources");
    let names: Vec<_> = sources
        .iter()
        .map(|(name, _)| {
            std::path::Path::new(name)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        names,
        ["Main.st"],
        "simulator must not execute TEST_PROGRAM declarations"
    );
}

#[test]
fn control_lessons_exclude_test_sources_and_preserve_explicit_test_discovery() {
    // Specification 38, Delivered loop bindings: native assertions are not PLC instances.
    let mut failures = Vec::new();
    for (lesson, project) in delivered_control_projects() {
        let entry = canonicalize_lossy(&project.join("src/Main.st"));
        let tests = project.join("src/tests.st");
        for include_globs in [Vec::new(), vec!["**/*.st".to_owned()]] {
            let options = SourceOptions {
                root: Some(project.to_string_lossy().into_owned()),
                include_globs,
                ..SourceOptions::default()
            };
            check_test_source_selection(&lesson, &entry, &tests, options, &mut failures);
        }
    }
    assert!(
        failures.is_empty(),
        "native test sources leaked into Simulator:\n{}",
        failures.join("\n")
    );
}

/// Specification 44 NX-16: the simulator builds each delivered lesson's sources for this computer,
/// with the conversion rules of the lesson's `vendor_profile` (specification 07 §2.10). The lessons
/// are built on several threads, each in a folder of its own, with the toolchain of §4.6 at
/// `TRUST_TOOLCHAIN`.
#[test]
fn control_lesson_simulator_sources_compile_as_delivered_programs() {
    let lessons = std::sync::Mutex::new(delivered_control_projects());
    let failures = std::sync::Mutex::new(Vec::new());
    let builders = std::thread::available_parallelism().map_or(4, |count| count.get().min(8));
    std::thread::scope(|scope| {
        for _ in 0..builders {
            scope.spawn(|| loop {
                let Some((lesson, project)) = lessons.lock().unwrap().pop() else {
                    break;
                };
                if let Err(error) = build_lesson(&project) {
                    failures.lock().unwrap().push(format!("{lesson}: {error}"));
                }
            });
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    assert!(
        failures.is_empty(),
        "delivered Simulator programs must compile:\n{}",
        failures.join("\n")
    );
}

/// Builds the lesson at `project` as the simulator builds it: the sources its entry selects, with
/// the conversion rules of its `vendor_profile`.
fn build_lesson(project: &std::path::Path) -> Result<(), String> {
    let entry = project.join("src/Main.st");
    let sources = collect_sources(
        entry.to_str().unwrap(),
        &SourceOptions {
            root: Some(project.to_string_lossy().into_owned()),
            include_globs: vec!["**/*.st".to_owned()],
            ..SourceOptions::default()
        },
    )
    .map_err(|error| error.to_string())?;
    let files = sources
        .into_iter()
        .map(|(name, text)| HarnessSourceFile::with_path(name, text))
        .collect::<Vec<_>>();
    let profile =
        trust_runtime::native::conversion_profile(project).map_err(|error| error.to_string())?;
    TestHarness::from_session(CompileSession::from_sources(files).with_conversion_profile(profile))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn delivered_control_projects() -> Vec<(String, std::path::PathBuf)> {
    let repository = std::env::var_os("TRUST_CONTROL_COURSE_SOURCE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let catalog_path = std::env::var_os("TRUST_CONTROL_COURSE_CATALOG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("examples/catalog.json"));
    let catalog: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(catalog_path).unwrap()).unwrap();
    let lessons: Vec<_> = catalog
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .filter(|path| path.starts_with("control/"))
        .map(|path| (path.to_owned(), repository.join("examples").join(path)))
        .collect();
    assert!(
        !lessons.is_empty(),
        "the delivered course must be discovered"
    );
    lessons
}

fn check_test_source_selection(
    lesson: &str,
    entry: &std::path::Path,
    tests: &std::path::Path,
    options: SourceOptions,
    failures: &mut Vec<String>,
) {
    let selected = collect_sources(entry.to_str().unwrap(), &options).unwrap();
    assert!(
        selected
            .iter()
            .any(|(name, _)| std::path::Path::new(name) == entry),
        "{lesson}: source selection must retain the delivered PLC entry"
    );
    if !tests.exists() {
        return; // Some lessons use external native integration suites.
    }
    let tests = canonicalize_lossy(tests);
    let original = std::fs::read_to_string(&tests).unwrap();
    let has_native_tests = original.lines().any(|line| {
        matches!(
            line.split_whitespace().next(),
            Some("TEST_PROGRAM" | "TEST_FUNCTION_BLOCK")
        )
    });
    if !has_native_tests {
        return; // Comment-only pointers to external suites are ordinary source text.
    }
    if selected
        .iter()
        .any(|(name, _)| std::path::Path::new(name) == tests)
    {
        failures.push(format!(
            "{lesson}: {:?} includes test-only source",
            options.include_globs
        ));
    }
    let with_tests = collect_sources(
        entry.to_str().unwrap(),
        &SourceOptions {
            ignore_pragmas: Some(Vec::new()),
            ..options
        },
    )
    .unwrap();
    assert!(
        with_tests
            .iter()
            .any(|(name, text)| std::path::Path::new(name) == tests && text == &original),
        "{lesson}: explicit test discovery must retain the complete original assertions"
    );
}
