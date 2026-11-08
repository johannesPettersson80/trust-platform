#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct TestSummary {
    passed: usize,
    failed: usize,
    errors: usize,
}

impl TestSummary {
    fn total(self) -> usize {
        self.passed + self.failed + self.errors
    }

    fn has_failures(self) -> bool {
        self.failed > 0 || self.errors > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestOutcome {
    Passed,
    Failed,
    Error,
}

impl TestOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
struct ExecutedTest {
    case: DiscoveredTest,
    outcome: TestOutcome,
    message: Option<String>,
    duration_ms: u64,
    /// The cycles a multi-cycle test ran (specification 22); `None` for a test that runs its body once.
    cycles: Option<CyclesRun>,
    /// The statement that raised the test's failure or error, as the runtime names it (specification 22):
    /// `None` for a test that passed, for a failure that no statement raised, and for a release image.
    failed_at: Option<trust_runtime::native::StatementAt>,
}
