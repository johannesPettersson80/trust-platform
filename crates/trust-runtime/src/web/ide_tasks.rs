//! IDE task job helpers.

#![allow(missing_docs)]

use super::*;

fn ide_task_to_snapshot(job: &IdeTaskJob) -> IdeTaskSnapshot {
    IdeTaskSnapshot {
        job_id: job.job_id,
        kind: job.kind.clone(),
        status: job.status.clone(),
        success: job.success,
        output: job.output.clone(),
        locations: parse_task_locations(job.output.as_str()),
        started_ms: job.started_ms,
        finished_ms: job.finished_ms,
    }
}

pub(super) fn parse_task_locations(output: &str) -> Vec<IdeTaskLocation> {
    let mut seen = std::collections::BTreeSet::new();
    let mut locations = Vec::new();
    for raw in output.lines() {
        let line = raw.trim();
        let line = line.strip_prefix("[stderr] ").unwrap_or(line);
        let Some(location) = parse_task_location_line(line) else {
            continue;
        };
        let key = format!("{}:{}:{}", location.path, location.line, location.column);
        if seen.insert(key) {
            locations.push(location);
        }
        if locations.len() >= 80 {
            break;
        }
    }
    locations
}

pub(super) fn parse_task_location_line(line: &str) -> Option<IdeTaskLocation> {
    let marker = ".st:";
    let marker_pos = line.find(marker)?;
    let path = line[..marker_pos + marker.len() - 1].trim().to_string();
    if path.is_empty() {
        return None;
    }

    let mut rest = &line[marker_pos + marker.len()..];
    let line_end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    if line_end == 0 {
        return None;
    }
    let line_number = rest[..line_end].parse::<u32>().ok()?;
    rest = &rest[line_end..];

    let mut column_number = 1_u32;
    if let Some(after_colon) = rest.strip_prefix(':') {
        let column_end = after_colon
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(after_colon.len());
        if column_end > 0 {
            column_number = after_colon[..column_end].parse::<u32>().unwrap_or(1);
            rest = &after_colon[column_end..];
        } else {
            rest = after_colon;
        }
    }

    let message = rest
        .trim_start_matches(':')
        .trim_start_matches('-')
        .trim()
        .to_string();
    Some(IdeTaskLocation {
        path,
        line: line_number,
        column: column_number,
        message,
    })
}

pub(super) fn ide_task_snapshot(
    store: Arc<Mutex<HashMap<u64, IdeTaskJob>>>,
    job_id: u64,
) -> Option<IdeTaskSnapshot> {
    let guard = store.lock().ok()?;
    guard.get(&job_id).map(ide_task_to_snapshot)
}

fn ide_task_append_output(store: &Arc<Mutex<HashMap<u64, IdeTaskJob>>>, job_id: u64, chunk: &str) {
    const MAX_OUTPUT_BYTES: usize = 512 * 1024;
    if let Ok(mut guard) = store.lock()
        && let Some(job) = guard.get_mut(&job_id)
    {
        job.output.push_str(chunk);
        if job.output.len() > MAX_OUTPUT_BYTES {
            let excess = job.output.len() - MAX_OUTPUT_BYTES;
            job.output.drain(..excess);
        }
    }
}

fn ide_task_finish(
    store: &Arc<Mutex<HashMap<u64, IdeTaskJob>>>,
    job_id: u64,
    success: bool,
    tail_message: &str,
) {
    if let Ok(mut guard) = store.lock()
        && let Some(job) = guard.get_mut(&job_id)
    {
        job.status = "completed".to_string();
        job.success = Some(success);
        job.finished_ms = Some(now_ms());
        if !tail_message.is_empty() {
            job.output.push_str(tail_message);
        }
    }
}

fn stream_pipe_to_job<R: std::io::Read + Send + 'static>(
    reader: R,
    prefix: &'static str,
    store: Arc<Mutex<HashMap<u64, IdeTaskJob>>>,
    job_id: u64,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines().map_while(Result::ok) {
            ide_task_append_output(&store, job_id, format!("{prefix}{line}\n").as_str());
        }
    })
}

fn configure_ide_task_command(command: &mut Command, kind: &str, project_root: &Path) {
    if kind == "build" {
        // Keep parity with CLI: `trust-runtime build --project <root>`
        // and let runtime resolve sources from `<project>/src` by default.
        command.arg("build").arg("--project").arg(project_root);
    } else if kind == "validate" {
        command.arg("validate").arg("--project").arg(project_root);
    } else {
        command.arg("test").arg("--project").arg(project_root);
    }
}

pub(super) fn start_ide_task_job(
    kind: &str,
    project_root: PathBuf,
    store: Arc<Mutex<HashMap<u64, IdeTaskJob>>>,
    seq: Arc<AtomicU64>,
) -> IdeTaskSnapshot {
    let job_id = seq.fetch_add(1, Ordering::Relaxed);
    let job = IdeTaskJob {
        job_id,
        kind: kind.to_string(),
        status: "running".to_string(),
        success: None,
        output: String::new(),
        started_ms: now_ms(),
        finished_ms: None,
    };
    if let Ok(mut guard) = store.lock() {
        guard.insert(job_id, job.clone());
    }

    let kind_text = kind.to_string();
    let store_bg = store.clone();
    thread::spawn(move || {
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(err) => {
                ide_task_finish(
                    &store_bg,
                    job_id,
                    false,
                    format!("[error] cannot resolve runtime executable: {err}\n").as_str(),
                );
                return;
            }
        };
        let mut command = Command::new(exe);
        configure_ide_task_command(&mut command, kind_text.as_str(), project_root.as_path());
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(project_root);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                ide_task_finish(
                    &store_bg,
                    job_id,
                    false,
                    format!("[error] failed to start task: {err}\n").as_str(),
                );
                return;
            }
        };

        let stdout_handle = child
            .stdout
            .take()
            .map(|stdout| stream_pipe_to_job(stdout, "", store_bg.clone(), job_id));
        let stderr_handle = child
            .stderr
            .take()
            .map(|stderr| stream_pipe_to_job(stderr, "[stderr] ", store_bg.clone(), job_id));

        let wait_result = child.wait();
        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        match wait_result {
            Ok(status) => {
                let success = status.success();
                let tail = if success {
                    "\n[done] task completed successfully\n".to_string()
                } else {
                    format!(
                        "\n[failed] task exited with code {}\n",
                        status.code().unwrap_or(-1)
                    )
                };
                ide_task_finish(&store_bg, job_id, success, tail.as_str());
            }
            Err(err) => {
                ide_task_finish(
                    &store_bg,
                    job_id,
                    false,
                    format!("\n[error] failed waiting for task: {err}\n").as_str(),
                );
            }
        }
    });
    ide_task_to_snapshot(&job)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_task_location_line_extracts_st_coordinates() {
        let parsed = parse_task_location_line("main.st:18:7 expected ';'")
            .expect("expected parsed location");
        assert_eq!(parsed.path, "main.st");
        assert_eq!(parsed.line, 18);
        assert_eq!(parsed.column, 7);
        assert!(parsed.message.contains("expected"));
    }

    #[test]
    fn parse_task_locations_deduplicates_repeated_hits() {
        let output = "\
[stderr] main.st:4:2 bad token\n\
[stderr] main.st:4:2 bad token\n\
[stderr] folder/aux.st:9:1 unresolved symbol\n";
        let parsed = parse_task_locations(output);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path, "main.st");
        assert_eq!(parsed[1].path, "folder/aux.st");
    }

    #[test]
    fn build_task_command_matches_cli_project_only_contract() {
        let mut command = Command::new("trust-runtime");
        configure_ide_task_command(&mut command, "build", Path::new("/tmp/project"));
        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(args, vec!["build", "--project", "/tmp/project"]);
    }
}
