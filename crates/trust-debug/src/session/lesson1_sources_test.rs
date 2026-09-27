use super::{
    canonicalize_lossy, collect_sources, CompileSession, HarnessSourceFile, SourceOptions,
};

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

#[test]
fn control_lesson_simulator_sources_compile_as_delivered_programs() {
    let mut failures = Vec::new();
    for (lesson, project) in delivered_control_projects() {
        let entry = project.join("src/Main.st");
        let sources = collect_sources(
            entry.to_str().unwrap(),
            &SourceOptions {
                root: Some(project.to_string_lossy().into_owned()),
                include_globs: vec!["**/*.st".to_owned()],
                ..SourceOptions::default()
            },
        )
        .unwrap();
        let files = sources
            .into_iter()
            .map(|(name, text)| HarnessSourceFile::with_path(name, text))
            .collect::<Vec<_>>();
        if let Err(error) = CompileSession::from_sources(files).build_runtime() {
            failures.push(format!("{lesson}: {error}"));
        }
    }
    assert!(
        failures.is_empty(),
        "delivered Simulator programs must compile:\n{}",
        failures.join("\n")
    );
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
