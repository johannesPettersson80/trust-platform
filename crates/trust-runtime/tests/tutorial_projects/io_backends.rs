//! Tutorial 17 (I/O backends and composed drivers) as its README writes it: each `io.toml` of its
//! steps on tutorial 13's project, and the loopback and simulated runs of steps 2 and 3.

use super::*;

/// The README of tutorial `folder`.
fn tutorial_readme(folder: &str) -> String {
    std::fs::read_to_string(repo_root().join(format!("examples/tutorials/{folder}/README.md")))
        .expect("a tutorial's README")
}

/// Tutorial 13's project, as its README writes it, with `io` as its `io.toml`.
fn tutorial_13_with(io: &str) -> Vec<(String, String)> {
    here_documents(&tutorial_readme("13_project_bootstrap_zero_to_first_app"))
        .into_iter()
        .map(|(file, text)| {
            let text = if file == "io.toml" {
                io.to_string()
            } else {
                text
            };
            (file, text)
        })
        .collect()
}

/// The `io.toml` files that tutorial 17's steps write, in the README's order.
fn io_files(readme: &str) -> Vec<String> {
    here_documents(readme)
        .into_iter()
        .filter(|(file, _)| file == "io.toml")
        .map(|(_, text)| text)
        .collect()
}

#[test]
fn tutorial_17_io_files_validate_and_the_mixed_form_is_refused() {
    // examples/tutorials/17_io_backends_and_multi_driver/README.md, steps 2 to 7 (specification 11
    // §6.5: validation checks each driver's parameters without opening a device; specification 32
    // §5: MQTT mappings resolve against the built program): on tutorial 13's built project each
    // io.toml validates, the Modbus/TCP and MQTT ones naming devices that do not exist, and the
    // composed one with its mapping of P1.LampOut. The Try-it's mixed form, `[io] driver` beside
    // `io.drivers`, is refused with the README's error. Catches a README file that the runtime
    // refuses, a validation that tries to reach a device, and a mixed form accepted.
    let readme = tutorial_readme("17_io_backends_and_multi_driver");
    let files = io_files(&readme);
    assert_eq!(files.len(), 6, "steps 2 to 7 write six io.toml files");
    let project = trust_test_support::unique_temp_path("tutorial-17");
    for (file, text) in here_documents(&tutorial_readme("13_project_bootstrap_zero_to_first_app")) {
        let path = project.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        std::fs::write(&path, text).expect("the file");
    }
    let run = |args: &[&str]| {
        trust_runtime_command()
            .args(args)
            .current_dir(&project)
            .output()
            .expect("run trust-runtime")
    };
    let built = run(&["build", "--project", ".", "--sources", "src"]);
    let mut validations = Vec::new();
    for text in &files {
        std::fs::write(project.join("io.toml"), text).expect("io.toml");
        validations.push((text.clone(), run(&["validate", "--project", "."])));
    }
    let expected = "use either io.driver/io.params or io.drivers, not both";
    let mixed = format!("[io]\ndriver = \"simulated\"\n\n{}", files[5]);
    std::fs::write(project.join("io.toml"), &mixed).expect("io.toml");
    let refused = run(&["validate", "--project", "."]);
    let _ = std::fs::remove_dir_all(&project);

    assert!(
        built.status.success(),
        "step 1's build: {}",
        describe(&built)
    );
    for (text, output) in validations {
        assert!(
            output.status.success(),
            "the README's io.toml validates:\n{text}\n{}",
            describe(&output)
        );
    }
    assert!(readme.contains(expected), "the README quotes `{expected}`");
    assert!(!refused.status.success(), "the mixed form is refused");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains(expected),
        "validate's error: {}",
        describe(&refused)
    );
}

#[test]
fn tutorial_17_loopback_feeds_the_output_back_and_simulated_does_not() {
    // examples/tutorials/17_io_backends_and_multi_driver/README.md, steps 2 and 3 (specification 11
    // §6.5, the loopback and simulated drivers): on `loopback`, %QX0.0 forced on makes %IX0.0 read
    // TRUE as well; released, both stay on, the program copying %IX0.0 to %QX0.0 and the loopback
    // %QX0.0 back; a written FALSE puts both out. On `simulated`, the same force leaves %IX0.0
    // FALSE, and a written TRUE turns both on and keeps them on. Catches a loopback that does not
    // copy the output into the input, a simulated driver that writes the inputs, and an input write
    // that the loopback's copy overrides.
    let readme = tutorial_readme("17_io_backends_and_multi_driver");
    let io = io_files(&readme);
    assert!(
        io[0].contains("driver = \"loopback\""),
        "step 2's io.toml: {}",
        io[0]
    );
    assert!(
        io[1].contains("driver = \"simulated\""),
        "step 3's io.toml: {}",
        io[1]
    );

    let loopback =
        ExampleRuntime::written("tutorial-17-loopback", tutorial_13_with(&io[0])).start();
    loopback.ctl_ok(&["io-force", "%QX0.0", "true"]);
    holds(
        &loopback,
        &[("%IX0.0", true), ("%QX0.0", true)],
        "step 2: the output forced on reaches the input",
    );
    loopback.ctl_ok(&["io-unforce", "%QX0.0"]);
    holds(
        &loopback,
        &[("%IX0.0", true), ("%QX0.0", true)],
        "step 2: released, the lamp holds itself on",
    );
    set(&loopback, "%IX0.0", false);
    holds(
        &loopback,
        &[("%IX0.0", false), ("%QX0.0", false)],
        "step 2: written FALSE, the lamp goes out",
    );
    drop(loopback);

    let simulated =
        ExampleRuntime::written("tutorial-17-simulated", tutorial_13_with(&io[1])).start();
    simulated.ctl_ok(&["io-force", "%QX0.0", "true"]);
    holds(
        &simulated,
        &[("%IX0.0", false), ("%QX0.0", true)],
        "step 3: the output forced on, the input stays",
    );
    simulated.ctl_ok(&["io-unforce", "%QX0.0"]);
    set(&simulated, "%IX0.0", true);
    holds(
        &simulated,
        &[("%IX0.0", true), ("%QX0.0", true)],
        "step 3: written TRUE, the lamp stays on",
    );
}
