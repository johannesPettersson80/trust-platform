//! CI-mode helpers (exit codes + shared JSON metadata).

/// Exit code: invalid project/configuration input.
pub const EXIT_INVALID_CONFIG: i32 = 10;
/// Exit code: build/compile failure.
pub const EXIT_BUILD_FAILED: i32 = 11;
/// Exit code: test assertion/runtime failure.
pub const EXIT_TEST_FAILED: i32 = 12;
/// Exit code: timeout (reserved for CI wrappers).
pub const EXIT_TIMEOUT: i32 = 13;
/// Exit code: unexpected/internal failure.
pub const EXIT_INTERNAL: i32 = 20;

/// Map runtime errors to stable CI exit codes.
#[must_use]
pub fn classify_error(message: &str) -> i32 {
    let msg = message.to_ascii_lowercase();

    if msg.contains("timeout") || msg.contains("timed out") {
        return EXIT_TIMEOUT;
    }

    if msg.contains("st test(s) failed")
        || msg.contains("assertionfailed")
        || msg.contains("assertion failed")
    {
        return EXIT_TEST_FAILED;
    }

    if msg.contains("compile")
        || msg.contains("sources directory not found")
        || msg.contains("no source files found")
    {
        return EXIT_BUILD_FAILED;
    }

    if msg.contains("invalid project folder")
        || msg.contains("invalid config")
        || msg.contains("missing runtime.toml")
        || msg.contains("missing program.stbc")
        || msg.contains("missing io.toml")
        || msg.contains("tcp control endpoint requires")
    {
        return EXIT_INVALID_CONFIG;
    }

    EXIT_INTERNAL
}

/// Classify an error with command context for CI commands.
#[must_use]
pub fn classify_error_with_command(message: &str, command: Option<&str>) -> i32 {
    let classified = classify_error(message);
    if classified != EXIT_INTERNAL {
        return classified;
    }
    match command {
        Some("build") => EXIT_BUILD_FAILED,
        Some("test") => EXIT_TEST_FAILED,
        Some("validate") => EXIT_INVALID_CONFIG,
        _ => EXIT_INTERNAL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_test_failure_code() {
        assert_eq!(classify_error("1 ST test(s) failed"), EXIT_TEST_FAILED);
        assert_eq!(
            classify_error("RuntimeError::AssertionFailed(ASSERT_EQUAL...)"),
            EXIT_TEST_FAILED
        );
    }

    #[test]
    fn classify_build_failure_code() {
        assert_eq!(
            classify_error("sources directory not found: /tmp/project/sources"),
            EXIT_BUILD_FAILED
        );
        assert_eq!(classify_error("compile failed"), EXIT_BUILD_FAILED);
    }

    #[test]
    fn classify_invalid_config_code() {
        assert_eq!(
            classify_error("invalid project folder '/tmp/p': missing src/ directory"),
            EXIT_INVALID_CONFIG
        );
        assert_eq!(
            classify_error("missing runtime.toml at /tmp/project/runtime.toml"),
            EXIT_INVALID_CONFIG
        );
    }

    #[test]
    fn classify_internal_code() {
        assert_eq!(classify_error("unexpected panic"), EXIT_INTERNAL);
    }

    #[test]
    fn classify_timeout_code() {
        assert_eq!(classify_error("operation timed out"), EXIT_TIMEOUT);
    }

    #[test]
    fn classify_with_command_falls_back_for_internal() {
        assert_eq!(
            classify_error_with_command("expected expression", Some("build")),
            EXIT_BUILD_FAILED
        );
        assert_eq!(
            classify_error_with_command("unexpected runtime issue", Some("test")),
            EXIT_TEST_FAILED
        );
        assert_eq!(
            classify_error_with_command("bad bundle", Some("validate")),
            EXIT_INVALID_CONFIG
        );
    }
}
