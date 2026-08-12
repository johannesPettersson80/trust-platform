use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DOC_ID: AtomicU64 = AtomicU64::new(0);

struct DocProject {
    root: PathBuf,
}

impl DocProject {
    fn new(label: &str) -> Self {
        let id = NEXT_DOC_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-library-doc-contract-{}-{label}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create library documentation project");
        Self { root }
    }

    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create documentation fixture parent");
        }
        fs::write(&path, contents).expect("write documentation fixture");
        path
    }

    fn config(&self, docs: &[&str]) -> ProjectConfig {
        let docs = docs
            .iter()
            .map(|path| format!("{path:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        ProjectConfig::from_contents(
            &self.root,
            Some(self.root.join("trust-lsp.toml")),
            &format!(
                r#"
[[libraries]]
name = "Vendor"
path = "vendor"
docs = [{docs}]
"#
            ),
        )
    }
}

impl Drop for DocProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

fn parse(contents: &str) -> FxHashMap<String, String> {
    let mut docs = FxHashMap::default();
    parse_markdown_docs(contents, &mut docs);
    docs
}

#[test]
fn prose_before_first_heading_is_ignored() {
    let docs = parse("Preamble text.\n\n# ABS\nAbsolute value.");
    assert_eq!(docs.len(), 1);
    assert_eq!(doc_for_name(&docs, "ABS"), Some("Absolute value."));
}

#[test]
fn atx_heading_levels_one_through_six_define_symbols() {
    let docs = parse(
        "# One\none\n## Two\ntwo\n### Three\nthree\n#### Four\nfour\n##### Five\nfive\n###### Six\nsix",
    );
    for (name, body) in [
        ("One", "one"),
        ("Two", "two"),
        ("Three", "three"),
        ("Four", "four"),
        ("Five", "five"),
        ("Six", "six"),
    ] {
        assert_eq!(doc_for_name(&docs, name), Some(body), "{name}");
    }
    assert_eq!(docs.len(), 6);
}

#[test]
fn heading_requires_whitespace_after_marker() {
    let docs = parse("# Valid\nfirst\n#not-a-heading\nstill first\n# Next\nsecond");
    assert_eq!(
        doc_for_name(&docs, "Valid"),
        Some("first\n#not-a-heading\nstill first")
    );
    assert!(doc_for_name(&docs, "not-a-heading").is_none());
    assert_eq!(doc_for_name(&docs, "Next"), Some("second"));
}

#[test]
fn seven_hashes_do_not_define_atx_heading() {
    let docs = parse("# Valid\nfirst\n####### NotAHeading\nstill first");
    assert_eq!(
        doc_for_name(&docs, "Valid"),
        Some("first\n####### NotAHeading\nstill first")
    );
    assert!(doc_for_name(&docs, "NotAHeading").is_none());
}

#[test]
fn tab_after_heading_marker_is_accepted() {
    let docs = parse("#\tTabbed\nDocumentation.");
    assert_eq!(doc_for_name(&docs, "Tabbed"), Some("Documentation."));
}

#[test]
fn heading_identity_is_trimmed_and_ascii_case_insensitive() {
    let docs = parse("##   Mixed_Name   \nDocumentation.");
    assert_eq!(doc_for_name(&docs, "mixed_name"), Some("Documentation."));
    assert_eq!(doc_for_name(&docs, "MIXED_NAME"), Some("Documentation."));
}

#[test]
fn body_preserves_line_order_and_paragraph_breaks() {
    let docs = parse(
        "# TON\n\nFirst paragraph line one.\nFirst paragraph line two.\n\nSecond paragraph.\n\n",
    );
    assert_eq!(
        doc_for_name(&docs, "TON"),
        Some("First paragraph line one.\nFirst paragraph line two.\n\nSecond paragraph.")
    );
}

#[test]
fn blank_heading_does_not_create_symbol() {
    let docs = parse("#   \nignored body\n# Real\nbody");
    assert_eq!(docs.len(), 1);
    assert_eq!(doc_for_name(&docs, "Real"), Some("body"));
}

#[test]
fn heading_without_body_does_not_create_symbol() {
    let docs = parse("# Empty\n\n# Real\nbody");
    assert!(doc_for_name(&docs, "Empty").is_none());
    assert_eq!(doc_for_name(&docs, "Real"), Some("body"));
}

#[test]
fn later_duplicate_heading_replaces_earlier_body() {
    let docs = parse("# ADD\nfirst\n# add\nsecond");
    assert_eq!(docs.len(), 1);
    assert_eq!(doc_for_name(&docs, "ADD"), Some("second"));
}

#[test]
fn empty_later_duplicate_does_not_erase_prior_body() {
    let docs = parse("# ADD\nfirst\n# add\n\n# MUL\nmultiply");
    assert_eq!(doc_for_name(&docs, "ADD"), Some("first"));
    assert_eq!(doc_for_name(&docs, "MUL"), Some("multiply"));
}

#[test]
fn crlf_input_has_canonical_lf_body() {
    let docs = parse("# ABS\r\nFirst.\r\n\r\nSecond.\r\n");
    assert_eq!(doc_for_name(&docs, "ABS"), Some("First.\n\nSecond."));
}

#[test]
fn later_configured_file_replaces_earlier_symbol() {
    let project = DocProject::new("file-precedence");
    project.write("docs/first.md", "# ABS\nFirst definition.");
    project.write("docs/second.md", "# abs\nSecond definition.");
    let config = project.config(&["docs/first.md", "docs/second.md"]);
    let docs = library_doc_map(&config);
    assert_eq!(doc_for_name(&docs, "ABS"), Some("Second definition."));
}

#[test]
fn missing_document_file_does_not_erase_other_entries() {
    let project = DocProject::new("missing-file");
    project.write("docs/real.md", "# ABS\nDefinition.");
    let config = project.config(&["docs/real.md", "docs/missing.md"]);
    let docs = library_doc_map(&config);
    assert_eq!(docs.len(), 1);
    assert_eq!(doc_for_name(&docs, "ABS"), Some("Definition."));
}

#[test]
fn non_utf8_document_file_contributes_no_entries() {
    let project = DocProject::new("non-utf8");
    project.write("docs/real.md", "# ABS\nDefinition.");
    project.write("docs/binary.md", [0xff, 0xfe, 0xfd]);
    let config = project.config(&["docs/real.md", "docs/binary.md"]);
    let docs = library_doc_map(&config);
    assert_eq!(docs.len(), 1);
    assert_eq!(doc_for_name(&docs, "ABS"), Some("Definition."));
}

#[test]
fn inline_hashes_and_fenced_text_remain_body_content() {
    let docs =
        parse("# CONCAT\nUse # inside prose.\n\n```text\n# not a markdown heading here\n```\n");
    assert_eq!(
        doc_for_name(&docs, "CONCAT"),
        Some("Use # inside prose.\n\n```text\n# not a markdown heading here\n```")
    );
}
