//! The README of a copied lesson (specification 25 §3.1, §3.2). The extension's former
//! `control-gallery-model` assertions about `adaptCopiedLessonReadme` live here now; each expected
//! text follows from the gallery and README each test writes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-copied-readme-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, text).expect("file");
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.0.join(relative)).expect("file")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A gallery with lessons `control/m/01-a` (sequence 1), `02-b` (sequence 2), `03-c` (no
/// lesson.json) and `04-d` (no lesson.json), and a copy of `01-a` in `copy/`.
fn gallery(readme: &str) -> Scratch {
    let scratch = Scratch::new();
    scratch.write(
        "gallery/manifest.json",
        r#"[
            {"id": "a", "title": "1. A", "description": "", "path": "control/m/01-a", "hardware": "none", "tags": ["control"]},
            {"id": "b", "title": "2. B", "description": "", "path": "control/m/02-b", "hardware": "none", "tags": ["control"]},
            {"id": "c", "title": "C", "description": "", "path": "control/m/03-c", "hardware": "none", "tags": ["control"]},
            {"id": "d", "title": "D", "description": "", "path": "control/m/04-d", "hardware": "none", "tags": ["control"]}
        ]"#,
    );
    scratch.write("gallery/control/m/01-a/lesson.json", r#"{"sequence": 1}"#);
    scratch.write("gallery/control/m/02-b/lesson.json", r#"{"sequence": 2}"#);
    scratch.write("copy/README.md", readme);
    scratch
}

fn adapt(scratch: &Scratch) -> bool {
    adapt_copied_readme(
        &scratch.0.join("gallery"),
        "control/m/01-a",
        &scratch.0.join("copy"),
    )
    .expect("adapted")
}

#[test]
fn a_link_to_a_gallery_lesson_names_how_to_open_it() {
    // The link text stays; the number is the target's lesson.json sequence, not the link's words.
    let scratch = gallery("Next: [Lesson 9](../02-b/README.md) and [the same](./../02-b).\n");
    assert!(adapt(&scratch));
    assert_eq!(
        scratch.read("copy/README.md"),
        "Next: Lesson 9 (to open it, use truST > Start from example and select Lesson 2) and the same (to open it, use truST > Start from example and select Lesson 2).\n"
    );
}

#[test]
fn without_lesson_json_the_link_text_gives_the_number_or_the_link_stays() {
    // `03-c` and `04-d` have no lesson.json: `Lesson 7` names its number, `Fourth` names none.
    let scratch = gallery("[lesson  7](../03-c/README.md) [Fourth](../04-d/README.md)\n");
    assert!(adapt(&scratch));
    assert_eq!(
        scratch.read("copy/README.md"),
        "lesson  7 (to open it, use truST > Start from example and select Lesson 7) [Fourth](../04-d/README.md)\n"
    );
}

#[test]
fn external_anchor_image_unknown_and_absolute_links_stay_as_written() {
    // Catches: a rewrite of a link the gallery cannot open, or of an image.
    let readme = "[Lesson 999](https://example.com/guide)\n[Lesson 998](../missing-lesson/README.md)\n![figure](../02-b/README.md)\n[Top](#top)\n[Root](/control/m/02-b)\n[Code](src/Main.st)\n[Mail](mailto:a@b.c)\n";
    let scratch = gallery(readme);
    assert!(!adapt(&scratch), "nothing to change");
    assert_eq!(scratch.read("copy/README.md"), readme);
}

#[test]
fn a_link_that_spans_lines_or_has_no_text_is_no_link() {
    let readme = "[Lesson\n2](../02-b/README.md) [](../02-b/README.md) [B](../02-b\n/README.md)\n";
    let scratch = gallery(readme);
    assert!(!adapt(&scratch));
    assert_eq!(scratch.read("copy/README.md"), readme);
}

#[test]
fn the_real_first_lesson_names_lesson_two_and_keeps_its_other_links() {
    // The packaged gallery's first lesson links ON/OFF feedback (`../02-cold-storage-on-off`),
    // sequence 2; its links to its own files stay. The gallery's README is not changed.
    let gallery = super::super::tests::packaged_gallery();
    let root = &gallery.0;
    let entry =
        "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy";
    let source = std::fs::read_to_string(root.join(entry).join("README.md")).expect("README");
    let scratch = Scratch::new();
    scratch.write("copy/README.md", &source);
    assert!(adapt_copied_readme(root, entry, &scratch.0.join("copy")).expect("adapted"));
    let copied = scratch.read("copy/README.md");
    assert!(copied.contains(
        "ON/OFF feedback (to open it, use truST > Start from example and select Lesson 2)"
    ));
    assert!(!copied.contains("../02-cold-storage-on-off/README.md"));
    assert!(copied.contains("[`src/Main.st`](src/Main.st)"));
    assert_eq!(
        std::fs::read_to_string(root.join(entry).join("README.md")).expect("README"),
        source
    );
}

#[test]
fn posix_normalization_resolves_dots_and_keeps_a_final_slash() {
    assert_eq!(normalize_posix("control/m/01-a/../02-b"), "control/m/02-b");
    assert_eq!(normalize_posix("control//m/./02-b/"), "control/m/02-b/");
    assert_eq!(normalize_posix("a/.."), ".");
    assert_eq!(normalize_posix("../x"), "../x");
}
