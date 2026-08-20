#[allow(unused_macros)]
macro_rules! generated_output_path {
    ($path:expr, $generated_root:expr $(,)?) => {{
        let path = ::std::path::PathBuf::from($path);
        let generated_root = ::std::path::PathBuf::from($generated_root);
        let metadata = ::std::fs::symlink_metadata(&path).unwrap_or_else(|error| {
            panic!(
                "inspect generated output {} under {}: {error}",
                path.display(),
                generated_root.display()
            )
        });
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "generated output oracle must be a regular non-symlink file: {}",
            path.display()
        );
        let canonical_root = generated_root.canonicalize().unwrap_or_else(|error| {
            panic!(
                "canonicalize generated output root {}: {error}",
                generated_root.display()
            )
        });
        let canonical_path = path.canonicalize().unwrap_or_else(|error| {
            panic!("canonicalize generated output {}: {error}", path.display())
        });
        assert!(
            canonical_path.starts_with(&canonical_root) && canonical_path != canonical_root,
            "generated output {} escaped root {}",
            canonical_path.display(),
            canonical_root.display()
        );
        canonical_path
    }};
}

#[allow(unused_macros)]
macro_rules! generated_test_root {
    ($path:expr $(,)?) => {{
        let path = ::std::path::PathBuf::from($path);
        let metadata = ::std::fs::symlink_metadata(&path).unwrap_or_else(|error| {
            panic!("inspect generated test root {}: {error}", path.display())
        });
        assert!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "generated test root must be a regular non-symlink directory: {}",
            path.display()
        );
        let canonical_temp = ::std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|error| panic!("canonicalize system temp directory: {error}"));
        let canonical_path = path.canonicalize().unwrap_or_else(|error| {
            panic!(
                "canonicalize generated test root {}: {error}",
                path.display()
            )
        });
        assert!(
            canonical_path.starts_with(&canonical_temp) && canonical_path != canonical_temp,
            "generated test root {} is outside system temp directory {}",
            canonical_path.display(),
            canonical_temp.display()
        );
        canonical_path
    }};
}
