use std::io::{BufRead, Write};

use anyhow::Context;

use crate::cli::PlantAction;

pub fn run_plant(action: PlantAction) -> anyhow::Result<()> {
    match action {
        PlantAction::Serve { project } => serve(project),
    }
}

fn serve(project: std::path::PathBuf) -> anyhow::Result<()> {
    let project = std::fs::canonicalize(&project)
        .with_context(|| format!("failed to resolve plant project {}", project.display()))?;
    if !project.is_dir() {
        anyhow::bail!("plant project is not a directory: {}", project.display());
    }
    let session = trust_runtime::control::OfflinePlantSession::new(project);
    let stdin = std::io::stdin();
    let mut stdout = std::io::BufWriter::new(std::io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line.context("failed to read plant request line")?;
        let response = match serde_json::from_str(&line) {
            Ok(request) => session.request_json(request),
            Err(error) => serde_json::json!({
                "id": 0,
                "ok": false,
                "error": format!("invalid offline plant request JSON: {error}"),
                "error_code": "malformed_request",
            }),
        };
        serde_json::to_writer(&mut stdout, &response).context("failed to write plant response")?;
        stdout.write_all(b"\n")?;
        // The extension waits for each response before issuing dependent work.
        stdout.flush()?;
    }
    Ok(())
}
