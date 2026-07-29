macro_rules! repository_source_tree_read_to_string {
    (
        $path_and_root:expr,
        roots = [$($root:literal),+ $(,)?],
        extension = $extension:literal $(,)?
    ) => {{
        let (path, repository_root) = $path_and_root;
        let path: &::std::path::Path = path.as_ref();
        let repository_root: &::std::path::Path = repository_root.as_ref();
        let result = (|| -> ::std::io::Result<::std::string::String> {
            let metadata = ::std::fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidInput,
                    "repository source oracle must be a regular non-symlink file",
                ));
            }
            let canonical_root = repository_root.canonicalize()?;
            let canonical_path = path.canonicalize()?;
            let relative = canonical_path.strip_prefix(&canonical_root).map_err(|_| {
                ::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidInput,
                    "repository source path escaped root",
                )
            })?;
            let allowed_root = false
                $(|| relative.starts_with(::std::path::Path::new($root)))+;
            let allowed_extension =
                relative.extension().and_then(|value| value.to_str())
                    == Some($extension);
            if !allowed_root || !allowed_extension {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidInput,
                    "repository source path is outside the declared tree",
                ));
            }
            ::std::fs::read_to_string(canonical_path)
        })();
        result
    }};
}
