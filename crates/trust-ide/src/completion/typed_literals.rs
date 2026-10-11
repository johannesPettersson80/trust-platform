fn typed_literal_completions() -> Vec<CompletionItem> {
    typed_literal_templates()
        .iter()
        .map(|template| {
            let prefix = template.primary_prefix();
            let label = format!("{prefix}#{}", template.value_label);
            let mut item = CompletionItem::new(label, CompletionKind::Snippet)
                .with_insert_text(format!("{prefix}#{}", template.value_snippet))
                .with_priority(15);
            if let Some(doc) = stdlib_docs::typed_literal_doc(prefix) {
                item.documentation = Some(SmolStr::new(doc));
            }
            item
        })
        .collect()
}

fn typed_literal_completions_with_context(
    context: Option<&TypedLiteralContext>,
) -> Vec<CompletionItem> {
    let Some(context) = context else {
        return typed_literal_completions();
    };
    typed_literal_completions_for_context(context)
}

#[derive(Debug, Clone)]
struct TypedLiteralTemplate {
    prefixes: &'static [&'static str],
    value_label: &'static str,
    value_snippet: &'static str,
}

impl TypedLiteralTemplate {
    fn primary_prefix(&self) -> &'static str {
        self.prefixes[0]
    }

    fn matches_prefix(&self, prefix: &str) -> bool {
        self.prefixes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(prefix))
    }
}

fn typed_literal_templates() -> &'static [TypedLiteralTemplate] {
    &[
        TypedLiteralTemplate {
            prefixes: &["T", "TIME", "LT", "LTIME"],
            value_label: "1s",
            value_snippet: "${1:1s}",
        },
        TypedLiteralTemplate {
            prefixes: &["DATE", "D", "LDATE", "LD"],
            value_label: "2024-01-15",
            value_snippet: "${1:2024-01-15}",
        },
        TypedLiteralTemplate {
            prefixes: &["TOD", "TIME_OF_DAY", "LTOD", "LTIME_OF_DAY"],
            value_label: "14:30:00",
            value_snippet: "${1:14:30:00}",
        },
        TypedLiteralTemplate {
            prefixes: &["DT", "DATE_AND_TIME", "LDT", "LDATE_AND_TIME"],
            value_label: "2024-01-15-14:30:00",
            value_snippet: "${1:2024-01-15-14:30:00}",
        },
        TypedLiteralTemplate {
            prefixes: &["BOOL"],
            value_label: "TRUE",
            value_snippet: "${1:TRUE}",
        },
        TypedLiteralTemplate {
            prefixes: &["SINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["INT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["DINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["LINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["USINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["UINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["UDINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["ULINT"],
            value_label: "0",
            value_snippet: "${1:0}",
        },
        TypedLiteralTemplate {
            prefixes: &["REAL"],
            value_label: "0.0",
            value_snippet: "${1:0.0}",
        },
        TypedLiteralTemplate {
            prefixes: &["LREAL"],
            value_label: "0.0",
            value_snippet: "${1:0.0}",
        },
        TypedLiteralTemplate {
            prefixes: &["BYTE"],
            value_label: "16#FF",
            value_snippet: "${1:16#FF}",
        },
        TypedLiteralTemplate {
            prefixes: &["WORD"],
            value_label: "16#FFFF",
            value_snippet: "${1:16#FFFF}",
        },
        TypedLiteralTemplate {
            prefixes: &["DWORD"],
            value_label: "16#FFFF_FFFF",
            value_snippet: "${1:16#FFFF_FFFF}",
        },
        TypedLiteralTemplate {
            prefixes: &["LWORD"],
            value_label: "16#FFFF_FFFF_FFFF_FFFF",
            value_snippet: "${1:16#FFFF_FFFF_FFFF_FFFF}",
        },
        TypedLiteralTemplate {
            prefixes: &["STRING"],
            value_label: "'text'",
            value_snippet: "'${1:text}'",
        },
        TypedLiteralTemplate {
            prefixes: &["WSTRING"],
            value_label: "\"text\"",
            value_snippet: "\"${1:text}\"",
        },
        TypedLiteralTemplate {
            prefixes: &["CHAR"],
            value_label: "'A'",
            value_snippet: "'${1:A}'",
        },
        TypedLiteralTemplate {
            prefixes: &["WCHAR"],
            value_label: "\"A\"",
            value_snippet: "\"${1:A}\"",
        },
    ]
}

#[derive(Debug, Clone)]
struct TypedLiteralContext {
    prefix: SmolStr,
    prefix_text: SmolStr,
    value_range: TextRange,
}

fn typed_literal_completion_context(
    context: &IdeContext<'_>,
    position: TextSize,
) -> Option<TypedLiteralContext> {
    let token = find_token_at_position(&context.root, position)?;
    let mut prefix_token: Option<SyntaxToken> = None;
    let mut value_range: Option<TextRange> = None;

    if token.kind() == SyntaxKind::TypedLiteralPrefix {
        prefix_token = Some(token.clone());
        value_range = Some(TextRange::new(
            token.text_range().end(),
            token.text_range().end(),
        ));
    } else if let Some(prev) = previous_non_trivia_token(&token)
        && prev.kind() == SyntaxKind::TypedLiteralPrefix
    {
        prefix_token = Some(prev);
        if is_typed_literal_value_token(token.kind()) {
            value_range = Some(token.text_range());
        } else {
            value_range = Some(TextRange::new(position, position));
        }
    }

    if let Some(prefix_token) = prefix_token {
        let prefix_text = prefix_token.text().trim_end_matches('#');
        return Some(TypedLiteralContext {
            prefix: SmolStr::new(prefix_text),
            prefix_text: SmolStr::new(prefix_text),
            value_range: value_range?,
        });
    }

    if token.text().contains('#') {
        let text = token.text();
        if let Some(hash_idx) = text.find('#') {
            let prefix_text = &text[..hash_idx];
            let start = token.text_range().start() + TextSize::from((hash_idx + 1) as u32);
            return Some(TypedLiteralContext {
                prefix: SmolStr::new(prefix_text),
                prefix_text: SmolStr::new(prefix_text),
                value_range: TextRange::new(start, token.text_range().end()),
            });
        }
    }

    None
}

fn typed_literal_completions_for_context(context: &TypedLiteralContext) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for template in typed_literal_templates() {
        if !template.matches_prefix(context.prefix.as_str()) {
            continue;
        }
        let label = format!("{}#{}", context.prefix_text, template.value_label);
        let mut item = CompletionItem::new(label, CompletionKind::Snippet)
            .with_text_edit(CompletionTextEdit {
                range: context.value_range,
                new_text: SmolStr::new(template.value_snippet),
            })
            .with_priority(15);
        if let Some(doc) = stdlib_docs::typed_literal_doc(context.prefix.as_str()) {
            item.documentation = Some(SmolStr::new(doc));
        }
        items.push(item);
    }
    items
}

fn is_typed_literal_value_token(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::IntLiteral
            | SyntaxKind::RealLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::WideStringLiteral
            | SyntaxKind::TimeLiteral
            | SyntaxKind::DateLiteral
            | SyntaxKind::TimeOfDayLiteral
            | SyntaxKind::DateAndTimeLiteral
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
            | SyntaxKind::Ident
    )
}
