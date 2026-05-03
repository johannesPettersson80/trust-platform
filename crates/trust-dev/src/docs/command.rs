pub fn run_docs(
    project: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    format: DocsFormat,
) -> anyhow::Result<()> {
    let project_root = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let sources_root = resolve_sources_root(&project_root, None)?;

    let sources = load_sources(&project_root, &sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let (items, diagnostics) = collect_api_items(&sources);
    let output_root = out_dir.unwrap_or_else(|| project_root.join("docs").join("api"));
    std::fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "failed to create documentation output directory '{}'",
            output_root.display()
        )
    })?;

    let mut written = Vec::new();
    if matches!(format, DocsFormat::Markdown | DocsFormat::Both) {
        let markdown = render_markdown(&items, &diagnostics);
        let path = output_root.join("api.md");
        std::fs::write(&path, markdown)
            .with_context(|| format!("failed to write '{}'", path.display()))?;
        written.push(path);
    }

    if matches!(format, DocsFormat::Html | DocsFormat::Both) {
        let html = render_html(&items, &diagnostics);
        let path = output_root.join("api.html");
        std::fs::write(&path, html)
            .with_context(|| format!("failed to write '{}'", path.display()))?;
        written.push(path);
    }

    println!(
        "{}",
        style::success(format!(
            "Generated documentation for {} API item(s) in {}",
            items.len(),
            output_root.display()
        ))
    );
    for path in &written {
        println!(" - {}", path.display());
    }

    if diagnostics.is_empty() {
        println!("{}", style::success("No documentation tag diagnostics."));
    } else {
        println!(
            "{}",
            style::warning(format!(
                "Generated with {} documentation diagnostic(s):",
                diagnostics.len()
            ))
        );
        for diagnostic in diagnostics {
            println!(
                " - {}:{} {}",
                diagnostic.file.display(),
                diagnostic.line,
                diagnostic.message
            );
        }
    }

    Ok(())
}

