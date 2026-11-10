/// Where a retain file lies (specification 11, sections 6.7 and 6.8.0a): beside `runtime.toml` in a
/// project folder, and in the store folder for a runtime started on a program store's `current`
/// link, where a retain file that an earlier version left in the `current` bundle is moved at the
/// start.
mod place_tests {
    use super::*;
    use crate::deployment::program_store::{replace_symlink, BUNDLES_DIR, CURRENT_LINK};

    /// A program store in a fresh folder with the bundle `a`, and `current` naming it. Returns the
    /// store folder and the `current` link.
    fn store_on_bundle_a(label: &str) -> (PathBuf, PathBuf) {
        let store = trust_test_support::unique_temp_path(&format!("retain-place-{label}"));
        let bundle = store.join(BUNDLES_DIR).join("a");
        fs::create_dir_all(&bundle).expect("create the bundle");
        let current = store.join(CURRENT_LINK);
        replace_symlink(&current, &bundle).expect("point current at the bundle");
        (store, current)
    }

    /// Wrong behaviour caught: a store runtime's relative retain file resolved through `current`,
    /// so that it lies in the bundle that `current` names and an install leaves it behind; or a
    /// project folder's or an absolute retain file moved.
    #[test]
    fn a_relative_retain_file_lies_beside_runtime_toml_or_in_the_store_folder() {
        let (store, current) = store_on_bundle_a("rule");
        let project = trust_test_support::unique_temp_path("retain-place-project");
        let absolute = store.join("elsewhere").join("retain.bin");

        assert_eq!(
            retain_file_path(&current, Path::new("retain.bin")),
            store.join("retain.bin")
        );
        assert_eq!(
            retain_file_path(&project, Path::new("retain.bin")),
            project.join("retain.bin")
        );
        assert_eq!(retain_file_path(&current, &absolute), absolute);
        let _ = fs::remove_dir_all(&store);
    }

    /// Wrong behaviour caught: after an upgrade, the start of a store runtime looks for its retain
    /// file in the store folder, finds none and starts cold, while the values an earlier version
    /// saved lie in the `current` bundle; or a retain file already in the store folder is replaced
    /// by an older one from a bundle.
    #[test]
    fn the_start_moves_a_retain_file_left_in_the_current_bundle_into_the_store_folder() {
        let (store, current) = store_on_bundle_a("move");
        let left_in_bundle = store.join(BUNDLES_DIR).join("a").join("retain.bin");
        fs::write(&left_in_bundle, b"saved by an earlier version").expect("write the old file");

        let path = prepare_retain_file(&current, Path::new("retain.bin")).expect("prepare");

        assert_eq!(path, store.join("retain.bin"));
        assert_eq!(
            fs::read(&path).expect("the moved file"),
            b"saved by an earlier version"
        );
        assert!(!left_in_bundle.exists(), "the bundle keeps no retain file");

        fs::write(&left_in_bundle, b"older").expect("write a stale bundle file");
        let again = prepare_retain_file(&current, Path::new("retain.bin")).expect("prepare again");
        assert_eq!(again, path);
        assert_eq!(
            fs::read(&path).expect("the store's file"),
            b"saved by an earlier version",
            "the store's own retain file is kept"
        );
        let _ = fs::remove_dir_all(&store);
    }
}
