//! Whether a CONFIGURATION can start a debug session (specification 25 §11.6). Each test writes its
//! project; the expected problem follows from the files written.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-run-entry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("project");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(&path, text).expect("file");
        path
    }

    fn problem(&self, program: &Path) -> Option<String> {
        let options = SourceOptionsArgs {
            runtime_root: Some(self.0.clone()),
            runtime_include_globs: vec!["**/*.{st,ST,pou,POU}".to_owned()],
            ..SourceOptionsArgs::default()
        };
        entry_problem(program, &options, &|path: &Path| {
            std::fs::read_to_string(path)
        })
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CONFIG: &str = "CONFIGURATION Config\nRESOURCE Res ON PLC\n  TASK T (INTERVAL := T#10ms, PRIORITY := 1);\n  PROGRAM P1 WITH T : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";

#[test]
fn a_configuration_of_a_declared_program_can_start() {
    let project = Project::new();
    project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let config = project.write("src/config.st", CONFIG);
    assert_eq!(project.problem(&config), None);
}

#[test]
fn an_unknown_program_type_is_named() {
    // `Mian` is a typo of `Main`. Catches: a check that never compares the types.
    let project = Project::new();
    project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let config = project.write("src/config.st", &CONFIG.replace(": Main", ": Mian"));
    assert_eq!(
        project.problem(&config).as_deref(),
        Some("Unknown PROGRAM type(s): Mian. Check that the file defining them is in the workspace and included in runtime sources.")
    );
}

#[test]
fn a_program_in_a_runtime_ignored_file_is_unknown_to_the_simulator() {
    // The simulator does not compile a file marked `@trustlsp:runtime-ignore` (specification 38 §3),
    // so a CONFIGURATION of its program cannot start. Catches: a check over every workspace file.
    let project = Project::new();
    project.write(
        "src/Main.st",
        "// @trustlsp:runtime-ignore\nPROGRAM Main\nEND_PROGRAM\n",
    );
    let config = project.write("src/config.st", CONFIG);
    assert!(project
        .problem(&config)
        .is_some_and(|problem| problem.starts_with("Unknown PROGRAM type(s): Main.")));
}

#[test]
fn a_configuration_without_program_entries_is_named() {
    let project = Project::new();
    let config = project.write(
        "src/config.st",
        "CONFIGURATION Config\nRESOURCE Res ON PLC\n  TASK T (INTERVAL := T#10ms, PRIORITY := 1);\nEND_RESOURCE\nEND_CONFIGURATION\n",
    );
    assert_eq!(
        project.problem(&config).as_deref(),
        Some("CONFIGURATION has no PROGRAM entries. Add a PROGRAM binding.")
    );
}

#[test]
fn a_namespace_qualified_type_is_declared_by_the_program_in_that_namespace() {
    // `Plant.Main` names the PROGRAM Main inside NAMESPACE Plant. Catches: the former text pattern,
    // which knew only `Main` and called `Plant.Main` unknown.
    let project = Project::new();
    project.write(
        "src/Main.st",
        "NAMESPACE Plant\nPROGRAM Main\nEND_PROGRAM\nEND_NAMESPACE\n",
    );
    let config = project.write("src/config.st", &CONFIG.replace(": Main", ": Plant.Main"));
    assert_eq!(project.problem(&config), None);
    let config = project.write("src/config.st", &CONFIG.replace(": Main", ": Other.Main"));
    assert!(
        project.problem(&config).is_some(),
        "another namespace's Main is not Plant's"
    );
}

#[test]
fn a_test_program_and_an_instance_name_declare_no_program_type() {
    // `TEST_PROGRAM Main` is a test, and `PROGRAM Main : Other` names an instance: neither declares
    // the PROGRAM type Main. Catches: the former text pattern, which took the instance name `P1` of a
    // `PROGRAM P1 : …` entry for a declared type.
    let project = Project::new();
    project.write("src/Tests.st", "TEST_PROGRAM Main\nEND_TEST_PROGRAM\n");
    project.write("src/Other.st", "PROGRAM Other\nEND_PROGRAM\n");
    let config = project.write(
        "src/config.st",
        "CONFIGURATION Config\nRESOURCE Res ON PLC\n  PROGRAM Main : Other;\n  PROGRAM P2 : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n",
    );
    assert!(project
        .problem(&config)
        .is_some_and(|problem| problem.starts_with("Unknown PROGRAM type(s): Main.")));
}

#[test]
fn a_program_of_a_local_package_dependency_is_declared() {
    // The simulator compiles `[dependencies]` sources as `trust-runtime build` resolves them.
    let project = Project::new();
    project.write(
        "app/trust-lsp.toml",
        "[dependencies]\nLib = { path = \"../lib\" }\n",
    );
    project.write("lib/trust-lsp.toml", "[package]\nversion = \"0.1.0\"\n");
    project.write("lib/src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let config = project.write("app/src/config.st", CONFIG);
    let options = SourceOptionsArgs {
        runtime_root: Some(project.0.join("app")),
        runtime_include_globs: vec!["**/*.st".to_owned()],
        ..SourceOptionsArgs::default()
    };
    assert_eq!(
        entry_problem(&config, &options, &|path: &Path| std::fs::read_to_string(
            path
        )),
        None
    );
}

#[test]
fn an_open_editor_text_counts_instead_of_its_file() {
    // The file on disk declares `Main`; the editor's unsaved text renamed it. Catches: a check that
    // reads the disk while the debug adapter receives the editor's text.
    let project = Project::new();
    let main = project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let config = project.write("src/config.st", CONFIG);
    // The file as the runtime's source collection names it, the name the reader is asked by.
    let main = trust_runtime::runtime_sources::canonicalize_lossy(&main);
    let options = SourceOptionsArgs {
        runtime_root: Some(project.0.clone()),
        runtime_include_globs: vec!["**/*.st".to_owned()],
        ..SourceOptionsArgs::default()
    };
    let edited = |path: &Path| {
        if path == main {
            Ok("PROGRAM Renamed\nEND_PROGRAM\n".to_owned())
        } else {
            std::fs::read_to_string(path)
        }
    };
    assert!(entry_problem(&config, &options, &edited)
        .is_some_and(|problem| problem.starts_with("Unknown PROGRAM type(s): Main.")));
}

fn disk(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

const CONFIG_ONLY: &str =
    "CONFIGURATION Config\nRESOURCE Res ON PLC\n  PROGRAM P1 : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";

#[test]
fn a_configuration_file_declares_a_configuration_outside_skipped_folders() {
    // A comment that names CONFIGURATION, a PROGRAM file and a copy under node_modules are no
    // CONFIGURATION files. Catches: the former text pattern, which took any file that mentions the
    // word.
    let project = Project::new();
    let config = project.write("src/config.st", CONFIG_ONLY);
    project.write("src/Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    project.write("src/notes.st", "(* The CONFIGURATION comes later. *)\n");
    project.write("node_modules/pkg/config.st", CONFIG_ONLY);
    project.write("target/out/config.st", CONFIG_ONLY);
    let found = configuration_files(std::slice::from_ref(&project.0), &disk);
    assert_eq!(found, std::slice::from_ref(&config));
    let (chosen, note) =
        choose_configuration(&found, std::slice::from_ref(&project.0), None, None, true);
    assert_eq!(
        (chosen, note),
        (Some(config), None),
        "one file is taken even when the user could pick"
    );
}

#[test]
fn an_automatic_start_takes_the_remembered_then_the_active_folders_then_the_first() {
    // Folder A holds two CONFIGURATION files, folder B one; the active file is in B. Catches: a choice
    // that ignores the remembered file or the active folder, or one that asks during an automatic
    // start.
    let a = Project::new();
    let b = Project::new();
    let a_one = a.write("one.st", CONFIG_ONLY);
    let a_two = a.write("two.st", CONFIG_ONLY);
    let b_three = b.write("three.st", CONFIG_ONLY);
    let active = b.write("Main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let folders = [a.0.clone(), b.0.clone()];
    let mut found = configuration_files(&folders, &disk);
    found.sort();
    assert_eq!(found.len(), 3);
    let (chosen, note) = choose_configuration(&found, &folders, Some(&a_two), Some(&active), false);
    assert_eq!(
        (chosen, note),
        (Some(a_two.clone()), None),
        "the remembered file"
    );
    let (chosen, note) = choose_configuration(&found, &folders, None, Some(&active), false);
    assert_eq!(
        chosen.as_ref(),
        Some(&b_three),
        "the only one in the active file's folder"
    );
    assert_eq!(
        note,
        Some(format!(
            "Multiple CONFIGURATION files found; using {}.",
            b_three.display()
        ))
    );
    let (chosen, _) = choose_configuration(&found, &folders, None, None, false);
    assert_eq!(chosen, Some(found[0].clone()), "the first by path");
    assert!(found[0] == a_one || found[0] == b_three);
    let (chosen, note) = choose_configuration(&found, &folders, None, Some(&active), true);
    assert_eq!(
        (chosen, note),
        (None, None),
        "an interactive start lets the user pick"
    );
}

#[test]
fn the_default_configuration_runs_a_program_of_the_active_file_else_of_the_sources() {
    // Catches: a default CONFIGURATION of a TEST_PROGRAM, or one that guesses among several programs
    // while the user could pick.
    let project = Project::new();
    let active = project.write(
        "src/Plant.st",
        "PROGRAM Filling\nEND_PROGRAM\nTEST_PROGRAM Checks\nEND_TEST_PROGRAM\nPROGRAM Draining\nEND_PROGRAM\n",
    );
    let options = SourceOptionsArgs {
        runtime_root: Some(project.0.clone()),
        ..SourceOptionsArgs::default()
    };
    let answer = program_candidates(Some(&active), &options, false, &disk).expect("programs");
    assert_eq!(answer["fromActive"], json!(true));
    assert_eq!(answer["chosen"]["name"], json!("Filling"));
    let names: Vec<&str> = answer["programs"]
        .as_array()
        .expect("programs")
        .iter()
        .filter_map(|program| program["name"].as_str())
        .collect();
    assert_eq!(names, ["Filling", "Draining"]);
    let answer = program_candidates(Some(&active), &options, true, &disk).expect("programs");
    assert_eq!(
        answer["chosen"],
        Value::Null,
        "two programs: the user picks"
    );

    // Without programs in the active file: the sources', the first by name chosen with a note.
    let notes = project.write("src/Notes.st", "FUNCTION Helper : INT\nEND_FUNCTION\n");
    project.write("src/Zone.st", "PROGRAM Zone\nEND_PROGRAM\n");
    let answer = program_candidates(Some(&notes), &options, false, &disk).expect("programs");
    assert_eq!(answer["fromActive"], json!(false));
    assert_eq!(answer["chosen"]["name"], json!("Draining"));
    assert_eq!(
        answer["note"],
        json!("Multiple PROGRAM types found; using Draining.")
    );
}

#[test]
fn without_any_program_no_default_configuration_is_offered() {
    let project = Project::new();
    project.write("src/Lib.st", "FUNCTION Helper : INT\nEND_FUNCTION\n");
    let options = SourceOptionsArgs {
        runtime_root: Some(project.0.clone()),
        ..SourceOptionsArgs::default()
    };
    assert_eq!(
        program_candidates(None, &options, false, &disk)
            .err()
            .as_deref(),
        Some("No PROGRAM declarations found to create a configuration.")
    );
}

#[test]
fn the_default_configuration_takes_the_resource_and_cycle_of_runtime_toml() {
    // A project whose runtime.toml names resource MainRes with a 20 ms cycle: the CONFIGURATION
    // declares that RESOURCE, which a build requires (specification 11 §6.8.1d), and runs its task
    // every cycle. A second default goes to configuration_2.st. Catches: `RESOURCE Res` refused by
    // Compile, a 100 ms task in a 10 ms project, or an existing file overwritten.
    let parent = Project::new();
    let project = trust_runtime::new_project::create_project(&parent.0, "plant", false)
        .expect("a new project");
    let runtime = std::fs::read_to_string(project.join("runtime.toml")).expect("runtime.toml");
    let resource = runtime
        .lines()
        .find_map(|line| line.strip_prefix("name = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the resource name")
        .to_owned();
    std::fs::write(
        project.join("runtime.toml"),
        runtime.replace("cycle_interval_ms = 10", "cycle_interval_ms = 20"),
    )
    .expect("runtime.toml");
    let first = create_configuration(&project, "Main").expect("created");
    assert_eq!(first, project.join("configuration.st"));
    assert_eq!(
        std::fs::read_to_string(&first).expect("text"),
        format!("CONFIGURATION Conf\n  RESOURCE {resource} ON PLC\n    TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\n  END_RESOURCE\nEND_CONFIGURATION\n")
    );
    let second = create_configuration(&project, "Plant.Main").expect("created");
    assert_eq!(second, project.join("configuration_2.st"));
    assert!(std::fs::read_to_string(&second)
        .expect("text")
        .contains("    PROGRAM P1 WITH MainTask : Plant.Main;\n"));
    assert!(create_configuration(&project, "Main; END_RESOURCE").is_err());
}

#[test]
fn without_runtime_toml_the_default_configuration_uses_res_and_ten_milliseconds() {
    // Ten milliseconds is the simulator's cycle without runtime.toml (specification 13).
    let project = Project::new();
    let path = create_configuration(&project.0, "Main").expect("created");
    let text = std::fs::read_to_string(path).expect("text");
    assert!(text.contains("  RESOURCE Res ON PLC\n"), "{text}");
    assert!(
        text.contains("(INTERVAL := T#10ms, PRIORITY := 1)"),
        "{text}"
    );
}

#[test]
fn the_default_configuration_goes_to_the_programs_folder_else_the_active_one_else_the_first() {
    // The extension's former `selectWorkspaceFolderPathForMode` cases: an interactive start lets the
    // user pick among several folders; an automatic one takes the active file's folder, else the
    // first. The chosen PROGRAM's own folder wins over both. Catches: a CONFIGURATION written into
    // another folder than its program's.
    let folders = [PathBuf::from("/a"), PathBuf::from("/b")];
    let active = PathBuf::from("/b/src/Main.st");
    assert_eq!(
        configuration_folder(None, Some(&active), &folders, true),
        (None, true)
    );
    assert_eq!(
        configuration_folder(None, Some(&active), &folders, false),
        (Some(PathBuf::from("/b")), false)
    );
    assert_eq!(
        configuration_folder(None, Some(Path::new("/missing/Main.st")), &folders, false),
        (Some(PathBuf::from("/a")), false)
    );
    assert_eq!(
        configuration_folder(
            Some(Path::new("/a/src/P.st")),
            Some(&active),
            &folders,
            true
        ),
        (Some(PathBuf::from("/a")), false)
    );
    assert_eq!(
        configuration_folder(None, None, &folders[..1], true),
        (Some(PathBuf::from("/a")), false)
    );
    assert_eq!(configuration_folder(None, None, &[], false), (None, false));
}

#[test]
fn a_file_declares_a_configuration_by_its_syntax_not_by_a_word() {
    let project = Project::new();
    let config = project.write("src/config.st", CONFIG_ONLY);
    let notes = project.write(
        "src/notes.st",
        "(* CONFIGURATION *)\nPROGRAM Main\nEND_PROGRAM\n",
    );
    let state = crate::state::ServerState::new();
    for (path, expected) in [(config, true), (notes, false)] {
        let answer = answer(&state, json!({ "action": "declares", "path": path })).expect("answer");
        assert_eq!(
            answer["configuration"],
            json!(expected),
            "{}",
            path.display()
        );
    }
}
