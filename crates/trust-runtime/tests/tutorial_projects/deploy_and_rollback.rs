//! Tutorial 14 (deploy and roll back) as its README writes it, on Unix: the tutorial's shell
//! commands and the program store's symbolic links (`deploy_cli_command` checks the store's
//! pointers on every platform).

use super::*;

/// A runtime started on a program store's `current` link, as tutorial 14 starts it, stopped when
/// dropped if it still runs.
struct StoreRuntime {
    child: std::process::Child,
    current: PathBuf,
}

impl StoreRuntime {
    /// Starts `trust-runtime run --project <current>` and waits until `ctl status` answers.
    fn start(current: PathBuf, log: &std::path::Path) -> Self {
        let out = std::fs::File::create(log).expect("the runtime's log");
        let child = trust_runtime_command()
            .args(["run", "--project"])
            .arg(&current)
            .stdout(out.try_clone().expect("the log twice"))
            .stderr(out)
            .spawn()
            .expect("start trust-runtime");
        let runtime = Self { child, current };
        let deadline = std::time::Instant::now() + STEP;
        while !runtime.ctl_output(&["status"]).status.success() {
            assert!(
                std::time::Instant::now() < deadline,
                "the runtime on {} did not answer `ctl status`: {}",
                runtime.current.display(),
                std::fs::read_to_string(log).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        runtime
    }

    fn ctl_output(&self, args: &[&str]) -> std::process::Output {
        trust_runtime_command()
            .args(["ctl", "--project"])
            .arg(&self.current)
            .args(args)
            .output()
            .expect("run trust-runtime ctl")
    }

    fn ctl(&self, args: &[&str]) -> String {
        let output = self.ctl_output(args);
        assert!(
            output.status.success(),
            "`ctl {}`: {}",
            args.join(" "),
            describe(&output)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Waits until `io-read` shows each address with its value.
    fn shows(&self, expected: &[(&str, &str)], what: &str) {
        let deadline = std::time::Instant::now() + STEP;
        loop {
            let reply: serde_json::Value =
                serde_json::from_str(self.ctl(&["io-read"]).trim()).expect("io-read's JSON");
            let snapshot = &reply["result"]["snapshot"];
            let value = |address: &str| {
                ["inputs", "outputs"]
                    .into_iter()
                    .filter_map(|area| snapshot[area].as_array())
                    .flatten()
                    .find(|entry| entry["address"] == address)
                    .and_then(|entry| entry["value"].as_str().map(str::to_string))
            };
            if expected
                .iter()
                .all(|(address, wanted)| value(address).as_deref() == Some(*wanted))
            {
                return;
            }
            assert!(std::time::Instant::now() < deadline, "{what}: {snapshot}");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Stops the program and starts it cold, which loads the bundle `current` names (specification
    /// 34, STATE-08).
    fn restart_on_current(&self) {
        self.ctl(&["shutdown"]);
        let deadline = std::time::Instant::now() + STEP;
        while !self.ctl(&["status"]).starts_with("state=stopped") {
            assert!(
                std::time::Instant::now() < deadline,
                "the program did not stop"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        self.ctl(&["restart", "cold"]);
        let deadline = std::time::Instant::now() + STEP;
        while !self.ctl(&["status"]).starts_with("state=running") {
            assert!(
                std::time::Instant::now() < deadline,
                "the program did not start again"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for StoreRuntime {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.ctl_output(&["shutdown", "--exit"]);
            std::thread::sleep(Duration::from_millis(300));
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

// The tutorial is a shell walkthrough with Unix paths, and the runtime runs on the store's
// symbolic link `current`; deploy_cli_command checks the store's pointers on every platform.
#[test]
fn tutorial_14_deploys_rolls_back_and_runs_the_revision_current_names_as_its_readme_writes_it() {
    // examples/tutorials/14_deploy_and_rollback/README.md on tutorial 13's project: deploy
    // revision A (the input copied to the output), run the runtime on the store's `current` link,
    // deploy revision B (`LampOut := NOT StartCmd;`), stop and start the program cold so that it
    // loads what `current` names, roll back, and stop and start it cold again. Specification 34
    // §11.2 and STATE-08: `deploy` moves `current` to the new bundle and `previous` to the one
    // before, `rollback` moves `current` back, and a restart that finds the program stopped loads
    // the bundle `current` names, while a running program keeps the one it started with. Catches a
    // deployment the runtime does not run, a restart that keeps the old revision, and a rollback
    // that leaves the new one in place.
    let readme = std::fs::read_to_string(
        repo_root().join("examples/tutorials/14_deploy_and_rollback/README.md"),
    )
    .expect("tutorial 14's README");
    for command in [
        "trust-runtime deploy --project . --root /tmp/trust-tutorial-14-deploy",
        "trust-runtime run --project /tmp/trust-tutorial-14-deploy/current",
        "trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current shutdown",
        "trust-runtime ctl --project /tmp/trust-tutorial-14-deploy/current restart cold",
        "trust-runtime rollback --root /tmp/trust-tutorial-14-deploy",
        "LampOut := NOT StartCmd;",
    ] {
        assert!(readme.contains(command), "the README names `{command}`");
    }

    let work = trust_test_support::unique_temp_path("tutorial-14");
    let project = work.join("project");
    let store = work.join("deploy");
    std::fs::create_dir_all(&store).expect("the deploy root");
    let files = here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"),
        )
        .expect("tutorial 13's README"),
    );
    for (file, text) in &files {
        let path = project.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        // Tests run side by side: the bench leaves the web interface off rather than on its port.
        let text = if file == "runtime.toml" {
            with_sections(text, "[runtime.web]\nenabled = false\n")
        } else {
            text.clone()
        };
        std::fs::write(&path, text).expect("the file");
    }
    let run = |args: &[&str]| {
        let output = trust_runtime_command()
            .args(args)
            .current_dir(&project)
            .output()
            .expect("run trust-runtime");
        assert!(
            output.status.success(),
            "`trust-runtime {}`: {}",
            args.join(" "),
            describe(&output)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let root = store.to_str().expect("a text path");
    run(&["build", "--project", ".", "--sources", "src"]);
    run(&["deploy", "--project", ".", "--root", root]);
    let first =
        std::fs::canonicalize(store.join("current")).expect("current after the first deploy");

    let runtime = StoreRuntime::start(store.join("current"), &work.join("runtime.log"));
    runtime.ctl(&["io-write", "%IX0.0", "true"]);
    runtime.shows(
        &[("%IX0.0", "Bool(true)"), ("%QX0.0", "Bool(true)")],
        "revision A copies",
    );

    let main = project.join("src").join("Main.st");
    let source = std::fs::read_to_string(&main).expect("src/Main.st");
    std::fs::write(
        &main,
        source.replace("LampOut := StartCmd;", "LampOut := NOT StartCmd;"),
    )
    .expect("revision B");
    run(&["build", "--project", ".", "--sources", "src"]);
    let summary = run(&["deploy", "--project", ".", "--root", root]);
    let second =
        std::fs::canonicalize(store.join("current")).expect("current after the second deploy");
    assert_ne!(first, second, "the second deploy moves current\n{summary}");
    assert_eq!(
        std::fs::canonicalize(store.join("previous")).expect("previous"),
        first,
        "previous names revision A"
    );
    runtime.shows(
        &[("%QX0.0", "Bool(true)")],
        "the running program keeps revision A",
    );
    runtime.restart_on_current();
    runtime.ctl(&["io-write", "%IX0.0", "true"]);
    runtime.shows(
        &[("%IX0.0", "Bool(true)"), ("%QX0.0", "Bool(false)")],
        "revision B inverts",
    );

    run(&["rollback", "--root", root]);
    assert_eq!(
        std::fs::canonicalize(store.join("current")).expect("current after the rollback"),
        first,
        "the rollback moves current back to revision A"
    );
    runtime.restart_on_current();
    runtime.ctl(&["io-write", "%IX0.0", "true"]);
    runtime.shows(
        &[("%IX0.0", "Bool(true)"), ("%QX0.0", "Bool(true)")],
        "revision A copies again",
    );
    drop(runtime);
    let _ = std::fs::remove_dir_all(&work);
}
