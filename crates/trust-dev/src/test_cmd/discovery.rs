fn load_sources(root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    for pattern in patterns {
        for entry in glob::glob(&format!("{}/{}", root.display(), pattern))
            .with_context(|| format!("invalid glob pattern for '{}'", root.display()))?
        {
            paths.insert(entry?);
        }
    }

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read source '{}'", path.display()))?;
        sources.push(LoadedSource { path, text });
    }
    Ok(sources)
}

fn discover_tests(sources: &[LoadedSource]) -> Vec<DiscoveredTest> {
    let mut tests = Vec::new();
    for source in sources {
        let parse = parser::parse(&source.text);
        let syntax = parse.syntax();
        for node in syntax.descendants() {
            let kind = match node.kind() {
                SyntaxKind::Program | SyntaxKind::FunctionBlock => test_kind_for_node(&node),
                _ => None,
            };
            let Some(kind) = kind else {
                continue;
            };
            let Some(name) = qualified_pou_name(&node) else {
                continue;
            };
            let byte_offset = node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| !token.kind().is_trivia())
                .map(|token| u32::from(token.text_range().start()))
                .unwrap_or_else(|| u32::from(node.text_range().start()));
            let line = line_for_offset(&source.text, byte_offset as usize);
            tests.push(DiscoveredTest {
                kind,
                name,
                file: source.path.clone(),
                byte_offset,
                line,
                source_line: source_line_for_offset(&source.text, byte_offset as usize),
            });
        }
    }
    tests.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.byte_offset.cmp(&right.byte_offset))
            .then(left.name.cmp(&right.name))
    });
    tests
}

fn test_kind_for_node(node: &SyntaxNode) -> Option<TestKind> {
    let first_token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())?;
    match first_token.kind() {
        SyntaxKind::KwTestProgram => Some(TestKind::Program),
        SyntaxKind::KwTestFunctionBlock => Some(TestKind::FunctionBlock),
        _ => None,
    }
}

fn qualified_pou_name(node: &SyntaxNode) -> Option<SmolStr> {
    let mut parts = Vec::new();
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    parts.push(name_part_from_name_node(&name_node)?);

    for ancestor in node.ancestors() {
        if ancestor.kind() != SyntaxKind::Namespace {
            continue;
        }
        if let Some(ns_name) = ancestor
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .and_then(|name_node| name_part_from_name_node(&name_node))
        {
            parts.push(ns_name);
        }
    }

    parts.reverse();
    Some(parts.join(".").into())
}

fn name_part_from_name_node(node: &SyntaxNode) -> Option<String> {
    let text = first_ident_token(node)?.text().trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident | SyntaxKind::KwEn | SyntaxKind::KwEno
            )
        })
}

fn line_for_offset(text: &str, byte_offset: usize) -> usize {
    let offset = byte_offset.min(text.len());
    text[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1
}

fn source_line_for_offset(text: &str, byte_offset: usize) -> Option<String> {
    let offset = byte_offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = text[offset..]
        .find('\n')
        .map(|rel| offset + rel)
        .unwrap_or(text.len());
    let line = text[line_start..line_end].trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

