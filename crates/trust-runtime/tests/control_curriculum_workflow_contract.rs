use std::path::{Path, PathBuf};

fn curriculum_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/control")
}

fn lesson_directories(root: &Path) -> Vec<PathBuf> {
    let mut lessons = Vec::new();
    for module in std::fs::read_dir(root).expect("read control curriculum root") {
        let module = module.expect("read control curriculum module").path();
        if !module.is_dir() {
            continue;
        }
        for lesson in std::fs::read_dir(&module).expect("read control curriculum module") {
            let lesson = lesson.expect("read control curriculum lesson").path();
            if lesson.join("lesson.json").is_file() {
                lessons.push(lesson);
            }
        }
    }
    lessons.sort_by_key(|lesson| {
        let descriptor: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(lesson.join("lesson.json")).expect("read lesson descriptor"),
        )
        .expect("parse lesson descriptor");
        descriptor["sequence"]
            .as_u64()
            .expect("numeric lesson sequence")
    });
    lessons
}

fn collect_write_symbols(value: &serde_json::Value, writes: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("direction").and_then(serde_json::Value::as_str) == Some("write") {
                writes.push(
                    object
                        .get("symbol")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("<missing symbol>")
                        .to_owned(),
                );
            }
            for child in object.values() {
                collect_write_symbols(child, writes);
            }
        }
        serde_json::Value::Array(array) => {
            for child in array {
                collect_write_symbols(child, writes);
            }
        }
        _ => {}
    }
}

#[test]
fn every_control_lesson_uses_the_lesson_8_monitor_test_authority_contract() {
    let lessons = lesson_directories(&curriculum_root());
    assert_eq!(
        lessons.len(),
        84,
        "the public curriculum must contain 84 lessons"
    );

    for (index, lesson) in lessons.iter().enumerate() {
        let number = index + 1;
        let descriptor: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(lesson.join("lesson.json")).expect("read lesson.json"),
        )
        .expect("parse lesson.json");
        assert_eq!(
            descriptor["sequence"],
            number,
            "lesson order at {}",
            lesson.display()
        );
        assert_eq!(
            descriptor["workflowStages"],
            serde_json::json!(["Monitor", "Test"]),
            "Lesson {number} must expose only Monitor then Test"
        );
        assert_eq!(
            descriptor["experiment"]["kind"], "observation",
            "Lesson {number}"
        );
        assert_eq!(descriptor["experiment"]["amplitude"], 0, "Lesson {number}");

        let control_loop: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(lesson.join("control-loop.json"))
                .expect("read control-loop.json"),
        )
        .expect("parse control-loop.json");
        let mut writes = Vec::new();
        collect_write_symbols(&control_loop, &mut writes);
        assert_eq!(
            writes,
            ["P1.ExperimentEnabled"],
            "Lesson {number} write authority"
        );
        assert_eq!(
            control_loop["experiment"]["kind"], "observation",
            "Lesson {number}"
        );
        assert_eq!(
            control_loop["experiment"]["amplitude"], 0,
            "Lesson {number}"
        );

        let readme = std::fs::read_to_string(lesson.join("README.md")).expect("read lesson README");
        assert!(
            readme.contains("1. **Monitor**"),
            "Lesson {number} Monitor instructions"
        );
        assert!(
            readme.contains("2. **Test**"),
            "Lesson {number} Test instructions"
        );
        assert!(
            readme.contains("Observe without excitation"),
            "Lesson {number} observation label"
        );
        assert!(
            readme.contains("P1.ExperimentEnabled"),
            "Lesson {number} authority name"
        );

        let source = std::fs::read_to_string(lesson.join("src/Main.st")).expect("read Main.st");
        assert!(
            source.contains("ExperimentEnabled : BOOL"),
            "Lesson {number} authority variable"
        );
        assert!(
            source.contains("P1.ExperimentEnabled AT"),
            "Lesson {number} mapped authority"
        );
    }

    let specification = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/specs/38-control-engineering-curriculum.md"),
    )
    .expect("read curriculum specification");
    assert!(specification.contains("exactly `Monitor -> Test`"));
    assert!(specification.contains("`P1.ExperimentEnabled`"));
    assert!(specification.contains("Observe without excitation"));
}
