#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestKind {
    Program,
    FunctionBlock,
}

impl TestKind {
    fn label(self) -> &'static str {
        match self {
            Self::Program => "TEST_PROGRAM",
            Self::FunctionBlock => "TEST_FUNCTION_BLOCK",
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct DiscoveredTest {
    kind: TestKind,
    name: SmolStr,
    file: PathBuf,
    byte_offset: u32,
    line: usize,
    source_line: Option<String>,
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
}

