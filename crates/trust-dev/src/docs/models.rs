#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiItemKind {
    Program,
    TestProgram,
    Function,
    FunctionBlock,
    TestFunctionBlock,
    Class,
    Interface,
    Method,
    Property,
}

impl ApiItemKind {
    fn label(self) -> &'static str {
        match self {
            Self::Program => "PROGRAM",
            Self::TestProgram => "TEST_PROGRAM",
            Self::Function => "FUNCTION",
            Self::FunctionBlock => "FUNCTION_BLOCK",
            Self::TestFunctionBlock => "TEST_FUNCTION_BLOCK",
            Self::Class => "CLASS",
            Self::Interface => "INTERFACE",
            Self::Method => "METHOD",
            Self::Property => "PROPERTY",
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct ApiParamDoc {
    name: SmolStr,
    description: String,
}

#[derive(Debug, Clone, Default)]
struct ApiDocTags {
    brief: Option<String>,
    details: Vec<String>,
    params: Vec<ApiParamDoc>,
    returns: Option<String>,
}

#[derive(Debug, Clone)]
struct ApiItem {
    kind: ApiItemKind,
    qualified_name: SmolStr,
    file: PathBuf,
    line: usize,
    tags: ApiDocTags,
    declared_params: Vec<SmolStr>,
    has_return: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocDiagnostic {
    file: PathBuf,
    line: usize,
    message: String,
}

#[derive(Debug, Clone)]
struct CommentBlock {
    lines: Vec<String>,
    start_line: usize,
}

enum CurrentTag {
    Brief,
    Detail,
    Param(usize),
    Return,
}

