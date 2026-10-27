#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_project_root(label: &str) -> std::path::PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("trust-debug-{label}-{id}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn parse_hit_condition_supports_basic_operators() {
        assert_eq!(parse_hit_condition("3"), Some(HitCondition::Equal(3)));
        assert_eq!(parse_hit_condition(">= 4"), Some(HitCondition::AtLeast(4)));
        assert_eq!(
            parse_hit_condition("> 5"),
            Some(HitCondition::GreaterThan(5))
        );
        assert_eq!(parse_hit_condition("==6"), Some(HitCondition::Equal(6)));
        assert!(parse_hit_condition("nope").is_none());
    }

    /// Specification 13, logpoints: `{expression}` encloses an expression and the text around it is
    /// kept. Catches a template whose expression or text is lost or merged.
    #[test]
    fn a_logpoint_template_splits_into_text_and_expressions() {
        assert_eq!(
            split_log_message("x={x}"),
            Ok(vec![
                LogText::Text("x=".to_string()),
                LogText::Expression("x".to_string()),
            ])
        );
        assert_eq!(
            split_log_message("{{x}} is {a.b}!"),
            Ok(vec![
                LogText::Text("{x} is ".to_string()),
                LogText::Expression("a.b".to_string()),
                LogText::Text("!".to_string()),
            ])
        );
    }

    /// Specification 13, logpoints: a brace that opens no expression, or one that is never closed,
    /// is refused, so the logpoint is not set. Catches a template that is logged as written.
    #[test]
    fn an_invalid_logpoint_template_is_refused() {
        assert_eq!(
            split_log_message("{"),
            Err("unterminated '{' in log message".to_string())
        );
        assert_eq!(
            split_log_message("x}"),
            Err("unmatched '}' in log message".to_string())
        );
        assert_eq!(
            split_log_message("{ }"),
            Err("empty log expression".to_string())
        );
    }

    // docs/specs/07-standard-functions.md 2.10: a debug session builds the program with the
    // conversion rules of its project's vendor_profile, as `trust-runtime run` does; a launch and
    // a reload both take the program's sources, and its profile, through `launch_sources`.
    // Catches a session that ignores the manifest's profile, or that runs a misspelled one under
    // the IEC rules instead of refusing it.
    #[test]
    fn session_reload_uses_the_conversion_rules_of_the_project_vendor_profile() {
        use trust_runtime::stdlib::conversions::ConversionProfile;

        let root = temp_project_root("vendor_profile");
        let path = root.join("main.st");
        std::fs::write(
            &path,
            "PROGRAM Main\nVAR\n    x : INT;\nEND_VAR\nx := INT#1;\nEND_PROGRAM\n",
        )
        .unwrap();
        let profile_after_reload = |vendor_profile: &str| {
            std::fs::write(
                root.join("trust-lsp.toml"),
                format!("[project]\nvendor_profile = \"{vendor_profile}\"\n"),
            )
            .unwrap();
            let mut session = DebugSession::new();
            session
                .launch_sources(path.to_string_lossy().as_ref())
                .map(|_| session.conversion_profile())
        };

        assert_eq!(
            profile_after_reload("codesys").unwrap(),
            ConversionProfile::Codesys
        );
        assert_eq!(
            profile_after_reload("twincat").unwrap(),
            ConversionProfile::Codesys
        );
        assert_eq!(
            profile_after_reload("beckhoff").unwrap(),
            ConversionProfile::Codesys
        );
        assert_eq!(
            profile_after_reload("mitsubishi").unwrap(),
            ConversionProfile::Iec
        );
        let error = profile_after_reload("codesis").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown vendor_profile 'codesis'"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_display_name_is_project_relative_but_path_stays_absolute() {
        let project_root = temp_project_root("source_display");
        let src = project_root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let main_path = src.join("main.st");
        let absolute_path = canonicalize_lossy(&main_path);
        std::fs::write(&absolute_path, "PROGRAM Main\nEND_PROGRAM\n").unwrap();

        let mut session = DebugSession::new();
        session.update_source_options(SourceOptionsUpdate {
            root: Some(project_root.to_string_lossy().to_string()),
            include_globs: None,
            exclude_globs: None,
            ignore_pragmas: None,
        });
        session.register_source(absolute_path.to_string_lossy().to_string(), 0, "");

        let source = session.source_for_file_id(0).expect("source for file id");
        assert_eq!(source.name.as_deref(), Some("src/main.st"));
        let source_path = source.path.as_deref().expect("DAP source path");
        assert!(
            std::path::Path::new(source_path).is_absolute(),
            "DAP path must stay absolute so VS Code can open the stack frame source"
        );
        assert_eq!(
            std::fs::canonicalize(source_path).expect("canonical DAP source path"),
            std::fs::canonicalize(&absolute_path).expect("canonical registered source path"),
            "DAP path must identify the registered source file"
        );
    }
}
