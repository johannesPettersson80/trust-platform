use super::*;
use crate::full_map::CheckStatus;
use crate::software_map::TargetSummary;

/// A throwaway repository in the temporary folder, removed when dropped.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("trust-unbuilt-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture root");
        Self { root }
    }

    fn file(&self, path: &str, text: &str) -> &Self {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("fixture file has a folder"))
            .expect("create fixture folder");
        fs::write(path, text).expect("write fixture file");
        self
    }

    fn member(dir: &str, targets: &[&str]) -> PackageSummary {
        PackageSummary {
            name: dir.to_string(),
            manifest_path: format!("{dir}/Cargo.toml"),
            targets: targets
                .iter()
                .map(|path| TargetSummary {
                    name: (*path).to_string(),
                    kind: vec!["lib".to_string()],
                    src_path: format!("{dir}/{path}"),
                })
                .collect(),
        }
    }

    fn check(
        &self,
        members: &[PackageSummary],
        allowances: &[UnbuiltSourceAllowance],
    ) -> FullMapCheck {
        check_unbuilt_sources(&self.root, members, allowances)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn allowance(path: &str, reason: &str) -> UnbuiltSourceAllowance {
    UnbuiltSourceAllowance {
        path: path.to_string(),
        owner: "runtime portability".to_string(),
        reason: reason.to_string(),
    }
}

const PACKAGE: &str = "[package]\nname = \"pkg\"\n";

#[test]
fn known_bad_file_outside_every_build_fails_until_it_is_declared() {
    let fixture = Fixture::new("known-bad");
    fixture
        .file("pkg/Cargo.toml", PACKAGE)
        .file("pkg/src/lib.rs", "mod kept;\n")
        .file("pkg/src/kept.rs", "")
        .file("pkg/src/lost.rs", "pub fn lost() {}\n");
    let members = [Fixture::member("pkg", &["src/lib.rs"])];

    let check = fixture.check(&members, &[]);
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(
        check.details,
        vec![
            "pkg/src/lost.rs is not part of any build: declare it with `mod`, delete it, or give \
             it an owner and reason in unbuilt_source_allowlist"
                .to_string()
        ]
    );

    fixture.file("pkg/src/lib.rs", "mod kept;\nmod lost;\n");
    let check = fixture.check(&members, &[]);
    assert_eq!(check.status, CheckStatus::Pass, "{:?}", check.details);
}

#[test]
fn follows_rustc_module_rules() {
    let fixture = Fixture::new("rules");
    fixture
        .file("pkg/Cargo.toml", PACKAGE)
        .file(
            "pkg/src/lib.rs",
            concat!(
                "mod flat;\n",
                "mod nested;\n",
                "mod r#type;\n",
                "#[cfg(any())]\nmod removed_by_cfg;\n",
                "mod inline {\n    mod inner;\n}\n",
                "include!(\"parts/included.rs\");\n",
                "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/table.rs\"));\n",
                "include!(concat!(env!(\"OUT_DIR\"), \"/built.rs\"));\n",
            ),
        )
        // A `name.rs` file declares its children in `name/`; its `#[path]` is
        // relative to its own folder, and an inline module adds a folder.
        .file(
            "pkg/src/flat.rs",
            "mod child;\n#[path = \"elsewhere/moved.rs\"]\nmod moved;\nmod block {\n    mod deep;\n}\n",
        )
        .file("pkg/src/flat/child.rs", "")
        .file("pkg/src/flat/block/deep.rs", "")
        // A `#[path]` file declares its children next to itself, like `mod.rs`.
        .file("pkg/src/elsewhere/moved.rs", "mod sibling;\n")
        .file("pkg/src/elsewhere/sibling.rs", "")
        .file("pkg/src/nested/mod.rs", "mod leaf;\n")
        .file("pkg/src/nested/leaf.rs", "")
        .file("pkg/src/type.rs", "")
        .file("pkg/src/inline/inner.rs", "")
        // An included file declares its modules relative to its own folder.
        .file("pkg/src/parts/included.rs", "mod own;\ninclude!(\"more.rs\");\n")
        .file("pkg/src/parts/own.rs", "")
        .file("pkg/src/parts/more.rs", "")
        .file("pkg/generated/table.rs", "");
    let members = [Fixture::member("pkg", &["src/lib.rs"])];

    let check = fixture.check(&members, &[]);
    assert_eq!(check.status, CheckStatus::Pass, "{:?}", check.details);
}

#[test]
fn allowlist_entries_must_stay_true() {
    let fixture = Fixture::new("allowlist");
    fixture
        .file("pkg/Cargo.toml", PACKAGE)
        .file("pkg/src/lib.rs", "mod built;\n")
        .file("pkg/src/built.rs", "")
        .file("pkg/src/parked.rs", "");
    let members = [Fixture::member("pkg", &["src/lib.rs"])];

    let check = fixture.check(
        &members,
        &[allowance("pkg/src/parked.rs", "kept for review")],
    );
    assert_eq!(check.status, CheckStatus::Pass, "{:?}", check.details);

    let check = fixture.check(&members, &[allowance("pkg/src/parked.rs", " ")]);
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(check.details.contains(
        &"pkg/src/parked.rs: an unbuilt_source_allowlist entry needs an owner and a reason"
            .to_string()
    ));

    let check = fixture.check(
        &members,
        &[
            allowance("pkg/src/parked.rs", "kept for review"),
            allowance("pkg/src/built.rs", "stale"),
            allowance("pkg/src/gone.rs", "stale"),
        ],
    );
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(
        check.details,
        vec![
            "pkg/src/built.rs is built again; remove its unbuilt_source_allowlist entry"
                .to_string(),
            "pkg/src/gone.rs no longer exists; remove its unbuilt_source_allowlist entry"
                .to_string(),
        ]
    );
}

#[test]
fn reads_packages_outside_the_workspace_and_skips_vendored_code() {
    let fixture = Fixture::new("nested");
    fixture
        .file("pkg/Cargo.toml", PACKAGE)
        .file("pkg/src/lib.rs", "")
        .file(
            "pkg/fuzz/Cargo.toml",
            "[package]\nname = \"pkg-fuzz\"\n\n[[bin]]\nname = \"frames\"\npath = \"fuzz_targets/frames.rs\"\n",
        )
        .file("pkg/fuzz/fuzz_targets/frames.rs", "mod helpers;\n")
        .file("pkg/fuzz/fuzz_targets/helpers.rs", "")
        .file(
            "firmware/board/Cargo.toml",
            "[package]\nname = \"board\"\nautobins = false\n\n[[bin]]\nname = \"board\"\npath = \"src/main.rs\"\n",
        )
        .file("firmware/board/src/main.rs", "")
        .file("firmware/board/src/bin/ignored.rs", "")
        .file("third_party/vendored/Cargo.toml", "[package]\nname = \"vendored\"\n")
        .file("third_party/vendored/src/unused.rs", "");
    let members = [Fixture::member("pkg", &["src/lib.rs"])];

    let check = fixture.check(&members, &[]);
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(
        check.details,
        vec![
            "firmware/board/src/bin/ignored.rs is not part of any build: declare it with `mod`, \
             delete it, or give it an owner and reason in unbuilt_source_allowlist"
                .to_string()
        ]
    );
}

#[test]
fn a_file_that_is_not_rust_fails_instead_of_hiding_its_modules() {
    let fixture = Fixture::new("unparsed");
    fixture
        .file("pkg/Cargo.toml", PACKAGE)
        .file("pkg/src/lib.rs", "mod broken;\n")
        .file("pkg/src/broken.rs", "fn (\n");
    let members = [Fixture::member("pkg", &["src/lib.rs"])];

    let check = fixture.check(&members, &[]);
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(
        check
            .details
            .iter()
            .any(|detail| detail.starts_with("pkg/src/broken.rs: not read as Rust")),
        "{:?}",
        check.details
    );
}
