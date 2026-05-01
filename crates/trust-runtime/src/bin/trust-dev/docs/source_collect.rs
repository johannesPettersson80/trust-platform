fn load_sources(project_root: &Path, root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
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
        let display = path
            .strip_prefix(project_root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        sources.push(LoadedSource {
            path: display,
            text,
        });
    }
    Ok(sources)
}

fn collect_api_items(sources: &[LoadedSource]) -> (Vec<ApiItem>, Vec<DocDiagnostic>) {
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();

    for source in sources {
        let parse = parser::parse(&source.text);
        let syntax = parse.syntax();
        let tokens = lexer::lex(&source.text);
        for node in syntax.descendants() {
            let Some(kind) = declaration_kind(&node) else {
                continue;
            };
            let Some(name) = declaration_name(&node) else {
                continue;
            };

            let declared_params = declared_param_names(&node);
            let has_return = declaration_has_return(&node, kind);
            let qualified_name = qualified_name(&node, &name);
            let Some(decl_offset) = first_non_trivia_token_start(&node) else {
                continue;
            };
            let decl_line = line_for_offset(&source.text, decl_offset);

            let mut tags = ApiDocTags::default();
            if let Some(comment) = leading_comment_block(&source.text, &tokens, decl_offset) {
                let (parsed, issues) = parse_doc_tags(
                    &comment,
                    &source.path,
                    kind,
                    qualified_name.as_str(),
                    &declared_params,
                    has_return,
                );
                tags = parsed;
                diagnostics.extend(issues);
            }

            items.push(ApiItem {
                kind,
                qualified_name,
                file: source.path.clone(),
                line: decl_line,
                tags,
                declared_params,
                has_return,
            });
        }
    }

    items.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.qualified_name.cmp(&right.qualified_name))
    });

    (items, diagnostics)
}
