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
    // `resolve_sources_root` canonicalizes its result. Canonicalize the project
    // identity used for display paths as well, otherwise platforms whose temp
    // directories have a symlinked prefix leak an absolute source path.
    let source_identity_root = std::fs::canonicalize(&project_root)
        .unwrap_or_else(|_| project_root.clone());
    let sources_root = resolve_sources_root(&source_identity_root, None)?;

    let sources = load_sources(&source_identity_root, &sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    validate_source_parses(&sources)?;
    let (items, diagnostics) = collect_api_items(&sources);
    let output_root = out_dir.unwrap_or_else(|| project_root.join("docs").join("api"));

    let mut documents = Vec::new();
    if matches!(format, DocsFormat::Markdown | DocsFormat::Both) {
        documents.push((
            output_root.join("api.md"),
            render_markdown(&items, &diagnostics),
        ));
    }

    if matches!(format, DocsFormat::Html | DocsFormat::Both) {
        documents.push((
            output_root.join("api.html"),
            render_html(&items, &diagnostics),
        ));
    }
    let written = publish_documents(&output_root, &documents)?;

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

fn validate_source_parses(sources: &[LoadedSource]) -> anyhow::Result<()> {
    for source in sources {
        let parse = parser::parse(&source.text);
        if let Some(error) = parse.errors().first() {
            anyhow::bail!(
                "failed to parse source '{}': {}",
                source.path.display(),
                error
            );
        }
    }
    Ok(())
}

fn publish_documents(
    output_root: &Path,
    documents: &[(PathBuf, String)],
) -> anyhow::Result<Vec<PathBuf>> {
    let created_output_root = ensure_output_root(output_root)?;
    let result = publish_documents_in_existing_root(documents);
    if result.is_err() && created_output_root {
        let _ = std::fs::remove_dir(output_root);
    }
    result
}

fn ensure_output_root(output_root: &Path) -> anyhow::Result<bool> {
    match std::fs::symlink_metadata(output_root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                anyhow::bail!(
                    "documentation output directory '{}' is a symbolic link",
                    output_root.display()
                );
            }
            if !metadata.is_dir() {
                anyhow::bail!(
                    "documentation output path '{}' is not a directory",
                    output_root.display()
                );
            }
            Ok(false)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(output_root).with_context(|| {
                format!(
                    "failed to create documentation output directory '{}'",
                    output_root.display()
                )
            })?;
            Ok(true)
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect documentation output directory '{}'",
                output_root.display()
            )
        }),
    }
}

struct StagedDocument {
    path: PathBuf,
    staged: PathBuf,
    backup: Option<PathBuf>,
    published: bool,
}

fn publish_documents_in_existing_root(
    documents: &[(PathBuf, String)],
) -> anyhow::Result<Vec<PathBuf>> {
    let mut entries = Vec::with_capacity(documents.len());
    for (path, _) in documents {
        validate_output_artifact(path)?;
        entries.push(StagedDocument {
            path: path.clone(),
            staged: temporary_artifact_path(path, "stage"),
            backup: None,
            published: false,
        });
    }

    for ((_, contents), entry) in documents.iter().zip(&entries) {
        if let Err(error) = stage_document(&entry.staged, contents) {
            cleanup_staged_documents(&entries);
            return Err(error);
        }
    }

    for index in 0..entries.len() {
        let path = entries[index].path.clone();
        if std::fs::symlink_metadata(&path).is_ok() {
            if let Err(error) = validate_output_artifact(&path) {
                rollback_documents(&entries);
                return Err(error);
            }
            let backup = temporary_artifact_path(&path, "backup");
            if let Err(error) = std::fs::rename(&path, &backup).with_context(|| {
                format!(
                    "failed to stage existing output artifact '{}'",
                    path.display()
                )
            }) {
                rollback_documents(&entries);
                return Err(error);
            }
            entries[index].backup = Some(backup);
        }
    }

    for index in 0..entries.len() {
        let staged = entries[index].staged.clone();
        let path = entries[index].path.clone();
        if let Err(error) = std::fs::rename(&staged, &path).with_context(|| {
            format!(
                "failed to publish documentation artifact '{}'",
                path.display()
            )
        }) {
            rollback_documents(&entries);
            return Err(error);
        }
        entries[index].published = true;
    }

    for entry in &entries {
        if let Some(backup) = &entry.backup {
            std::fs::remove_file(backup).with_context(|| {
                format!(
                    "failed to remove temporary output backup '{}'",
                    backup.display()
                )
            })?;
        }
    }

    let written = entries.iter().map(|entry| entry.path.clone()).collect();
    Ok(written)
}

fn validate_output_artifact(path: &Path) -> anyhow::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect output artifact '{}'", path.display()))
        }
    };
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "documentation output artifact '{}' is a symbolic link",
            path.display()
        );
    }
    if !metadata.is_file() {
        anyhow::bail!(
            "documentation output artifact '{}' is not a regular file",
            path.display()
        );
    }
    Ok(())
}

fn stage_document(path: &Path, contents: &str) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .with_context(|| {
            format!(
                "failed to stage documentation artifact '{}'",
                path.display()
            )
        })?;
    use std::io::Write as _;
    file.write_all(contents.as_bytes()).with_context(|| {
        format!(
            "failed to stage documentation artifact '{}'",
            path.display()
        )
    })?;
    Ok(())
}

fn temporary_artifact_path(path: &Path, role: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("api");
    path.with_file_name(format!(
        ".{name}.trust-dev-{role}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ))
}

fn cleanup_staged_documents(entries: &[StagedDocument]) {
    for entry in entries {
        let _ = std::fs::remove_file(&entry.staged);
    }
}

fn rollback_documents(entries: &[StagedDocument]) {
    for entry in entries.iter().rev() {
        if entry.published {
            let _ = std::fs::remove_file(&entry.path);
        }
        if let Some(backup) = &entry.backup {
            let _ = std::fs::rename(backup, &entry.path);
        }
        let _ = std::fs::remove_file(&entry.staged);
    }
}
