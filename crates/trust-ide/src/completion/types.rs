/// The kind of completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A keyword.
    Keyword,
    /// A function.
    Function,
    /// A function block.
    FunctionBlock,
    /// A method.
    Method,
    /// A property.
    Property,
    /// A variable.
    Variable,
    /// A constant.
    Constant,
    /// A type.
    Type,
    /// An enum value.
    EnumValue,
    /// A namespace.
    Namespace,
    /// A snippet.
    Snippet,
}

/// A completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The label shown in the completion list.
    pub label: SmolStr,
    /// The kind of completion.
    pub kind: CompletionKind,
    /// Additional detail (e.g., type signature).
    pub detail: Option<SmolStr>,
    /// Documentation.
    pub documentation: Option<SmolStr>,
    /// Text to insert (if different from label).
    pub insert_text: Option<SmolStr>,
    /// Text edit to apply (overrides insert_text when present).
    pub text_edit: Option<CompletionTextEdit>,
    /// Sort priority (lower = higher priority).
    pub sort_priority: u32,
}

impl CompletionItem {
    /// Creates a new completion item.
    pub fn new(label: impl Into<SmolStr>, kind: CompletionKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            documentation: None,
            insert_text: None,
            text_edit: None,
            sort_priority: 100,
        }
    }

    /// Sets the detail text.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<SmolStr>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Sets the documentation.
    #[must_use]
    pub fn with_documentation(mut self, doc: impl Into<SmolStr>) -> Self {
        self.documentation = Some(doc.into());
        self
    }

    /// Sets the insert text.
    #[must_use]
    pub fn with_insert_text(mut self, text: impl Into<SmolStr>) -> Self {
        self.insert_text = Some(text.into());
        self
    }

    /// Sets the text edit to apply.
    #[must_use]
    pub fn with_text_edit(mut self, edit: CompletionTextEdit) -> Self {
        self.text_edit = Some(edit);
        self
    }

    /// Sets the sort priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.sort_priority = priority;
        self
    }
}

/// Text edit for completion items.
#[derive(Debug, Clone)]
pub struct CompletionTextEdit {
    /// The range to replace.
    pub range: TextRange,
    /// The new text to insert.
    pub new_text: SmolStr,
}

/// Context for completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionContext {
    /// At the start of a statement.
    Statement,
    /// After a dot (member access).
    MemberAccess,
    /// After a colon (type context).
    TypeAnnotation,
    /// Inside a call (parameter).
    Argument,
    /// Top level (outside any POU).
    TopLevel,
    /// Inside a VAR block.
    VarBlock,
    /// Unknown/general context.
    General,
}
