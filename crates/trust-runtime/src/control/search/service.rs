use super::{
    contracts::Request,
    run_contracts::RunRequest,
    snapshot::Snapshot,
    worker::{WorkerInput, MAX_OUTPUT},
    workload::{Purpose, Workload},
};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_JOB: AtomicU64 = AtomicU64::new(1);
#[derive(Debug)]
struct Job {
    purpose: Purpose,
    id: String,
    fingerprint: String,
    revision: String,
    project: String,
    status: String,
    trials: Vec<Value>,
    count: usize,
    cancel: Arc<AtomicBool>,
    captured_samples: Arc<AtomicU64>,
    reason: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct SearchService {
    job: Arc<Mutex<Option<Job>>>,
    used_ids: Arc<Mutex<std::collections::HashSet<String>>>,
}
struct Lease;
impl Drop for Lease {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::SeqCst);
    }
}
impl SearchService {
    pub(super) fn start(&self, snapshot: Snapshot, request: Request) -> Result<Value, String> {
        self.start_workload(snapshot, Workload::Search(request))
    }
    pub(super) fn start_run(
        &self,
        snapshot: Snapshot,
        request: RunRequest,
    ) -> Result<Value, String> {
        self.start_workload(snapshot, Workload::Run(request))
    }
    fn start_workload(&self, snapshot: Snapshot, request: Workload) -> Result<Value, String> {
        let candidates = request.candidates()?;
        let purpose = request.purpose();
        if request.expected_revision() != snapshot.project_revision {
            return Err("project changed since review".into());
        }
        let mut guard = self.job.lock().map_err(|_| "search service unavailable")?;
        let mut used_ids = self
            .used_ids
            .lock()
            .map_err(|_| "operation identity registry unavailable")?;
        let fingerprint = request.fingerprint();
        if let Some(id) = request.client_id() {
            if let Some(job) = guard.as_ref().filter(|job| job.id == id) {
                if job.fingerprint != fingerprint
                    || job.revision != snapshot.project_revision
                    || job.project != snapshot.project_identity
                {
                    return Err("operation identity was reused with different inputs".into());
                }
                return Ok(
                    json!({"job_id":job.id,"status":job.status,"project_revision":job.revision}),
                );
            }
            if used_ids.contains(id) {
                return Err("operation already handled; retained result expired".into());
            }
            if used_ids.len() >= 4096 {
                return Err("operation identity registry is full".into());
            }
        }
        if guard
            .as_ref()
            .is_some_and(|job| matches!(job.status.as_str(), "queued" | "running"))
        {
            return Err("a search is already running".into());
        }
        if ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("another isolated search worker is running".into());
        }
        let generated_id = format!(
            "{}-{}-{}-{}",
            if purpose == Purpose::Search {
                "search"
            } else {
                "run"
            },
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
            NEXT_JOB.fetch_add(1, Ordering::Relaxed)
        );
        let id = request
            .client_id()
            .map(str::to_owned)
            .unwrap_or(generated_id);
        if request.client_id().is_some() {
            used_ids.insert(id.clone());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let captured_samples = Arc::new(AtomicU64::new(0));
        *guard = Some(Job {
            purpose,
            id: id.clone(),
            fingerprint,
            revision: snapshot.project_revision.clone(),
            project: snapshot.project_identity.clone(),
            status: "queued".into(),
            trials: Vec::new(),
            count: candidates.len(),
            cancel: cancel.clone(),
            captured_samples: captured_samples.clone(),
            reason: None,
        });
        let jobs = self.job.clone();
        let count = candidates.len();
        std::thread::spawn(move || {
            let _lease = Lease;
            let deadline = Instant::now() + Duration::from_millis(request.wall_budget_ms());
            if let Some(job) = jobs.lock().unwrap().as_mut() {
                job.status = "running".into();
            }
            let mut retained_bytes = 0_usize;
            let mut output_exhausted = false;
            for (index, parameters) in candidates.into_iter().enumerate() {
                if cancel.load(Ordering::SeqCst) || Instant::now() >= deadline {
                    break;
                }
                let input = request.input(snapshot.clone(), index, parameters.clone());
                let limit = deadline
                    .min(Instant::now() + Duration::from_millis(request.per_trial_wall_ms()));
                let mut trial=run_child(&input,&cancel,limit,&captured_samples).unwrap_or_else(|reason|json!({"index":index,"parameters":parameters,"status":"failed","reason":reason,"score":null}));
                if purpose == Purpose::Search {
                    trial["index"] = json!(index);
                    trial["parameters"] = json!(parameters);
                }
                let bytes =
                    serde_json::to_vec(&trial).map_or(MAX_OUTPUT as usize, |bytes| bytes.len());
                if retained_bytes.saturating_add(bytes) > MAX_OUTPUT as usize - 65_536 {
                    output_exhausted = true;
                    if let Some(job) = jobs.lock().unwrap().as_mut() {
                        job.trials.push(json!({"index":index,"parameters":parameters,"status":"failed","reason":"total retained output budget exhausted; trial evidence omitted","score":null}));
                    }
                    break;
                }
                retained_bytes += bytes;
                if let Some(job) = jobs.lock().unwrap().as_mut() {
                    job.trials.push(trial);
                }
            }
            if let Some(job) = jobs.lock().unwrap().as_mut() {
                job.status = if cancel.load(Ordering::SeqCst) {
                    "cancelled"
                } else if output_exhausted
                    || (purpose == Purpose::Run
                        && job
                            .trials
                            .first()
                            .is_some_and(|result| result["status"] != "executed"))
                {
                    "failed"
                } else if job.trials.len() == count {
                    "completed"
                } else {
                    "failed"
                }
                .into();
                if job.status == "failed" {
                    job.reason = Some(
                        if purpose == Purpose::Run
                            && job
                                .trials
                                .first()
                                .is_some_and(|result| result["status"] != "executed")
                        {
                            job.trials[0]["reason"]
                                .as_str()
                                .unwrap_or("isolated Test execution failed")
                        } else if output_exhausted {
                            "total retained output budget exhausted; no ranking is available"
                        } else {
                            "total wall budget exhausted; remaining trials are unattempted"
                        }
                        .into(),
                    );
                }
            }
        });
        Ok(if purpose == Purpose::Search {
            json!({"job_id":id,"status":"queued","project_revision":snapshot_revision(&guard),"trial_count":count})
        } else {
            json!({"job_id":id,"status":"queued","project_revision":snapshot_revision(&guard)})
        })
    }
    pub(super) fn inspect(
        &self,
        id: &str,
        revision: &str,
        project: &str,
        cancel: bool,
        include_results: Option<bool>,
    ) -> Result<Value, String> {
        self.inspect_for(
            Purpose::Search,
            id,
            revision,
            project,
            cancel,
            include_results,
        )
    }
    pub(super) fn inspect_for(
        &self,
        purpose: Purpose,
        id: &str,
        revision: &str,
        project: &str,
        cancel: bool,
        include_results: Option<bool>,
    ) -> Result<Value, String> {
        let guard = self.job.lock().map_err(|_| "search service unavailable")?;
        let job = guard.as_ref().ok_or("search job not found")?;
        if job.purpose != purpose
            || job.id != id
            || job.revision != revision
            || job.project != project
        {
            return Err("search identity/project/revision mismatch".into());
        }
        if cancel && matches!(job.status.as_str(), "queued" | "running") {
            job.cancel.store(true, Ordering::SeqCst);
        }
        if include_results == Some(true) && matches!(job.status.as_str(), "queued" | "running") {
            return Err("detailed results require a terminal job state".into());
        }
        if purpose == Purpose::Run {
            let mut result = json!({"job_id":job.id,"project_revision":job.revision,"status":job.status,"cancel_requested":job.cancel.load(Ordering::SeqCst),"reason":job.reason,"source_basis":"loaded-runtime-sources","io_provenance":"isolated-simulation-no-physical-drivers"});
            result["captured_sample_count"] = json!(job.captured_samples.load(Ordering::SeqCst));
            if include_results != Some(false)
                && let Some(run) = job.trials.first().and_then(|trial| trial.get("run"))
            {
                result["run"] = run.clone();
            }
            return Ok(result);
        }
        let trials: Vec<Value> = if include_results == Some(false) {
            job.trials
                .iter()
                .map(|trial| {
                    let mut summary = serde_json::Map::new();
                    for key in [
                        "index",
                        "parameters",
                        "status",
                        "reason",
                        "score",
                        "score_unit",
                        "violations",
                    ] {
                        if let Some(value) = trial.get(key) {
                            summary.insert(key.into(), value.clone());
                        }
                    }
                    Value::Object(summary)
                })
                .collect()
        } else {
            job.trials.clone()
        };
        let mut ranking: Vec<_> = if job.status == "completed" {
            job.trials
                .iter()
                .filter(|t| t["status"] == "qualified")
                .collect()
        } else {
            Vec::new()
        };
        ranking.sort_by(|a, b| {
            a["score"]
                .as_f64()
                .unwrap()
                .total_cmp(&b["score"].as_f64().unwrap())
                .then_with(|| a["index"].as_u64().cmp(&b["index"].as_u64()))
        });
        Ok(
            json!({"job_id":job.id,"project_revision":job.revision,"status":job.status,"cancel_requested":job.cancel.load(Ordering::SeqCst),"trial_count":job.count,"trials":trials,"unattempted_trials":(job.trials.len()..job.count).collect::<Vec<_>>(),"ranking":ranking.iter().map(|t|t["index"].clone()).collect::<Vec<_>>(),"reason":job.reason,"source_basis":"loaded-runtime-sources","io_provenance":"isolated-simulation-no-physical-drivers"}),
        )
    }
}
fn snapshot_revision(guard: &Option<Job>) -> Option<&str> {
    guard.as_ref().map(|job| job.revision.as_str())
}
fn run_child(
    input: &WorkerInput,
    cancel: &AtomicBool,
    deadline: Instant,
    captured: &Arc<AtomicU64>,
) -> Result<Value, String> {
    let bytes = serde_json::to_vec(input).map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("worker snapshot exceeds wire limit".into());
    }
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .env_clear()
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(not(test))]
    command.arg("control-search-worker");
    #[cfg(test)]
    command
        .args([
            "--exact",
            "control::search::tests::finite_search_worker_entry",
            "--nocapture",
        ])
        .env("TRUST_SEARCH_TEST_WORKER", "1");
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("worker stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("worker stdout unavailable")?;
    let writer = std::thread::spawn(move || stdin.write_all(&bytes));
    let captured = captured.clone();
    let reader =
        std::thread::spawn(move || super::worker_checkpoint::read_output(stdout, &captured));
    let outcome = loop {
        if cancel.load(Ordering::SeqCst) || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(if cancel.load(Ordering::SeqCst) {
                "search cancelled".into()
            } else {
                "trial compilation/execution wall budget exhausted".into()
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!("isolated worker exited {status}"))
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.to_string());
            }
        }
    };
    let _ = writer.join();
    let output = reader.join().map_err(|_| "worker output reader failed")?;
    match outcome.and(output.result) {
        Ok(result) => Ok(result),
        Err(reason) => match (input, output.prefix) {
            (WorkerInput::Run(_), Some(run)) => Ok(json!({"status":"failed","reason":reason,
                "io_provenance":"isolated-simulation-no-physical-drivers","run":run})),
            _ => Err(reason),
        },
    }
}
