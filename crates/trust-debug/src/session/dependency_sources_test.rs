//! The simulator compiles a project's local package dependencies (`[dependencies]` in
//! `trust-lsp.toml`), as `trust-runtime build` and the language server do, so a project that
//! uses the Alarm or Control library from outside its own folder runs in VS Code's simulator.

use super::{collect_sources, CompileSession, HarnessSourceFile, SourceOptions};

#[test]
fn the_simulator_compiles_the_sources_of_the_project_dependencies() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project =
        std::env::temp_dir().join(format!("trust-debug-dependencies-{}", std::process::id()));
    std::fs::create_dir_all(project.join("src")).expect("src");
    std::fs::write(
        project.join("src/main.st"),
        std::fs::read_to_string(repo.join("examples/hmi_tank_level/src/Main.st"))
            .expect("the example's program"),
    )
    .expect("the program");
    let library = |name: &str| {
        repo.join("libraries")
            .join(name)
            .canonicalize()
            .expect("the library")
    };
    std::fs::write(
        project.join("trust-lsp.toml"),
        format!(
            "[project]\ninclude_paths = [\"src\"]\n\n[dependencies]\nControl = {{ path = {:?}, version = \"0.1.0\" }}\nAlarm = {{ path = {:?}, version = \"0.1.0\" }}\nHmi = {{ path = {:?}, version = \"0.1.0\" }}\n",
            library("control").to_string_lossy(),
            library("alarm").to_string_lossy(),
            library("hmi").to_string_lossy()
        ),
    )
    .expect("the manifest");
    let sources = collect_sources(
        project.join("src/main.st").to_str().unwrap(),
        &SourceOptions {
            root: Some(project.to_string_lossy().into_owned()),
            include_globs: vec!["**/*.{st,ST,pou,POU}".to_owned()],
            ..SourceOptions::default()
        },
    )
    .expect("the project's sources");
    let names: Vec<String> = sources
        .iter()
        .map(|(name, _)| {
            std::path::Path::new(name)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        names.iter().any(|name| name == "alarm.st")
            && names.iter().any(|name| name == "control.st"),
        "the Alarm and Control libraries' sources: {names:?}"
    );
    CompileSession::from_sources(
        sources
            .into_iter()
            .map(|(path, text)| HarnessSourceFile::with_path(path, text))
            .collect(),
    )
    .build_runtime()
    .expect("the program compiles with its dependencies");
    std::fs::remove_dir_all(project).ok();
}
