//! Performance and stress harnesses for trust-lsp.

#[cfg(test)]
mod tests {
    use crate::handlers::{completion, document_diagnostic, hover, index_workspace, rename};
    use crate::state::ServerState;
    use crate::test_support::test_client;
    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    #[cfg(target_os = "linux")]
    use std::process::Command;
    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(target_os = "linux")]
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};
    use tower_lsp::lsp_types::{
        CompletionParams, DocumentDiagnosticParams, HoverParams, Position, RenameParams,
        TextDocumentIdentifier, TextDocumentPositionParams, WorkspaceSymbolParams,
    };

    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    struct CountingAllocator;

    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    static ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    static DEALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    #[global_allocator]
    static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            // SAFETY: forwards to the system allocator with the same layout.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            // SAFETY: forwards to the system allocator with the original pointer/layout pair.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            // SAFETY: forwards to the system allocator reallocation contract.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[derive(Clone, Copy)]
    struct AllocSnapshot {
        alloc_calls: u64,
        alloc_bytes: u64,
        dealloc_bytes: u64,
    }

    fn position_at(source: &str, needle: &str) -> Position {
        let offset = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing needle '{needle}'"));
        let mut line = 0u32;
        let mut character = 0u32;
        for (idx, ch) in source.char_indices() {
            if idx >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                character = 0;
            } else {
                character += 1;
            }
        }
        Position::new(line, character)
    }

    fn env_usize(name: &str, default: usize) -> usize {
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    fn env_u64(name: &str, default: u64) -> u64 {
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    fn avg_duration(iterations: usize, mut f: impl FnMut()) -> Duration {
        let iterations = iterations.max(1);
        let start = Instant::now();
        for _ in 0..iterations {
            f();
        }
        let total = start.elapsed();
        Duration::from_nanos((total.as_nanos() / iterations as u128) as u64)
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn perf_state(source: &str) -> (ServerState, tower_lsp::lsp_types::Url) {
        let state = ServerState::new();
        let uri = tower_lsp::lsp_types::Url::parse("file:///perf/Main.st").unwrap();
        state.open_document(uri.clone(), 1, source.to_string());
        (state, uri)
    }

    #[cfg(target_os = "linux")]
    fn clk_tck() -> u64 {
        static CLK_TCK: OnceLock<u64> = OnceLock::new();
        *CLK_TCK.get_or_init(|| {
            Command::new("getconf")
                .arg("CLK_TCK")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|raw| raw.trim().parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(100)
        })
    }

    #[cfg(target_os = "linux")]
    fn process_cpu_millis() -> Option<u64> {
        let stat = fs::read_to_string("/proc/self/stat").ok()?;
        let close = stat.rfind(')')?;
        let rest = stat.get(close + 2..)?;
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // Fields after comm start at state(3). utime=14, stime=15 => indexes 11 and 12 here.
        let utime: u64 = fields.get(11)?.parse().ok()?;
        let stime: u64 = fields.get(12)?.parse().ok()?;
        Some(((utime + stime) * 1000) / clk_tck())
    }

    #[cfg(not(target_os = "linux"))]
    fn process_cpu_millis() -> Option<u64> {
        None
    }

    #[cfg(target_os = "linux")]
    fn process_rss_kb() -> Option<u64> {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        let line = status
            .lines()
            .find(|line| line.starts_with("VmRSS:"))?
            .split_whitespace()
            .nth(1)?;
        line.parse::<u64>().ok()
    }

    #[cfg(not(target_os = "linux"))]
    fn process_rss_kb() -> Option<u64> {
        None
    }

    #[cfg(all(target_os = "linux", feature = "perf_alloc_metrics"))]
    fn alloc_snapshot() -> AllocSnapshot {
        AllocSnapshot {
            alloc_calls: ALLOC_CALLS.load(Ordering::Relaxed),
            alloc_bytes: ALLOC_BYTES.load(Ordering::Relaxed),
            dealloc_bytes: DEALLOC_BYTES.load(Ordering::Relaxed),
        }
    }

    #[cfg(not(all(target_os = "linux", feature = "perf_alloc_metrics")))]
    fn alloc_snapshot() -> AllocSnapshot {
        AllocSnapshot {
            alloc_calls: 0,
            alloc_bytes: 0,
            dealloc_bytes: 0,
        }
    }

    fn percentile_duration(samples: &[Duration], percentile: u32) -> Duration {
        let mut sorted: Vec<Duration> = samples.to_vec();
        sorted.sort_unstable();
        if sorted.is_empty() {
            return Duration::from_millis(0);
        }
        let clamped = percentile.clamp(1, 100) as usize;
        let rank = ((sorted.len() - 1) * clamped) / 100;
        sorted[rank]
    }

    fn edit_loop_source(tick: usize) -> String {
        format!(
            "PROGRAM Main\nVAR\n    counter : INT;\nEND_VAR\ncounter := counter + {};\ncounter := coun\nEND_PROGRAM\n",
            tick % 1000
        )
    }

    #[test]
    #[ignore]
    fn perf_hover_budget() {
        let source = r#"
FUNCTION Foo : INT
VAR_INPUT
    a : INT;
END_VAR
Foo := a;
END_FUNCTION

PROGRAM Main
VAR
    x : INT;
END_VAR
x := Foo(1);
END_PROGRAM
"#;
        let (state, uri) = perf_state(source);
        let params = tower_lsp::lsp_types::HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_at(source, "Foo(1)"),
            },
            work_done_progress_params: Default::default(),
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 50);
        let budget_ms = env_u64("ST_LSP_PERF_HOVER_MS", 50);
        let avg = avg_duration(iterations, || {
            let _ = hover(&state, params.clone());
        });
        assert!(
            avg.as_millis() <= budget_ms as u128,
            "hover avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_completion_budget() {
        let source = r#"
FUNCTION Foo : INT
VAR_INPUT
    a : INT;
END_VAR
Foo := a;
END_FUNCTION

PROGRAM Main
VAR
    x : INT;
END_VAR
x := F
END_PROGRAM
"#;
        let (state, uri) = perf_state(source);
        let mut position = position_at(source, "x := F");
        position.character += 6;
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 50);
        let budget_ms = env_u64("ST_LSP_PERF_COMPLETION_MS", 200);
        let avg = avg_duration(iterations, || {
            let _ = completion(&state, params.clone());
        });
        assert!(
            avg.as_millis() <= budget_ms as u128,
            "completion avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_rename_budget() {
        let source = r#"
PROGRAM Main
VAR
    x : INT;
END_VAR
x := 1;
x := x + 1;
END_PROGRAM
"#;
        let (state, uri) = perf_state(source);
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_at(source, "x : INT"),
            },
            new_name: "counter".to_string(),
            work_done_progress_params: Default::default(),
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 20);
        let budget_ms = env_u64("ST_LSP_PERF_RENAME_MS", 100);
        let avg = avg_duration(iterations, || {
            let _ = rename(&state, params.clone());
        });
        assert!(
            avg.as_millis() <= budget_ms as u128,
            "rename avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_large_workspace_index_budget() {
        let file_count = env_usize("ST_LSP_PERF_STRESS_FILES", 10_000);
        let budget_ms = env_u64("ST_LSP_PERF_INDEX_MS", 5_000);
        let root = temp_dir("trust-lsp-perf");
        for idx in 0..file_count {
            let path = root.join(format!("Program{idx:05}.st"));
            let content = format!(
                "PROGRAM Program{idx:05}\nVAR\n    x : INT;\nEND_VAR\nx := {idx};\nEND_PROGRAM\n"
            );
            fs::write(&path, content).expect("write perf file");
        }

        let client = test_client();
        let state = ServerState::new();
        let root_uri = tower_lsp::lsp_types::Url::from_file_path(&root).unwrap();
        state.set_workspace_folders(vec![root_uri.clone()]);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let start = Instant::now();
        runtime.block_on(index_workspace(&client, &state));
        let elapsed = start.elapsed();

        let _ = fs::remove_dir_all(&root);

        assert!(
            elapsed.as_millis() <= budget_ms as u128,
            "indexing avg {:?} exceeded budget {}ms",
            elapsed,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_workspace_symbol_budget() {
        let source = r#"
PROGRAM Main
VAR
    counter : INT;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;
        let (state, _uri) = perf_state(source);
        let params = WorkspaceSymbolParams {
            query: "Main".to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 20);
        let budget_ms = env_u64("ST_LSP_PERF_WORKSPACE_SYMBOL_MS", 200);
        let client = test_client();
        let avg = avg_duration(iterations, || {
            let params = params.clone();
            runtime.block_on(async {
                let _ =
                    crate::handlers::workspace_symbol_with_progress(&client, &state, params).await;
            });
        });

        assert!(
            avg.as_millis() <= budget_ms as u128,
            "workspace/symbol avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_diagnostics_budget() {
        let declarations = env_usize("ST_LSP_PERF_DIAGNOSTIC_DECLS", 200);
        let mut source = String::from(
            r#"
TYPE MotorConfig : STRUCT
    speed : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    cfg : MotroConfig;
    flag : BOOL;
"#,
        );
        for idx in 0..declarations {
            source.push_str(&format!("    speedValue{idx} : DINT;\n"));
        }
        source.push_str(
            r#"END_VAR
    speadValue0 := 1;
    flag := 1;
"#,
        );
        for idx in 0..declarations {
            source.push_str(&format!("    speedValue{idx} := {idx};\n"));
        }
        source.push_str("END_PROGRAM\n");

        let (state, uri) = perf_state(&source);
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let iterations = env_usize("ST_LSP_PERF_ITERATIONS", 30);
        let budget_ms = env_u64("ST_LSP_PERF_DIAGNOSTICS_MS", 900);
        let avg = avg_duration(iterations, || {
            let _ = document_diagnostic(&state, params.clone());
        });

        assert!(
            avg.as_millis() <= budget_ms as u128,
            "diagnostics avg {:?} exceeded budget {}ms",
            avg,
            budget_ms
        );
    }

    #[test]
    #[ignore]
    fn perf_edit_loop_budget() {
        let initial = edit_loop_source(0);
        let (state, uri) = perf_state(&initial);

        let hover_params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_at(&initial, "counter +"),
            },
            work_done_progress_params: Default::default(),
        };

        let mut completion_pos = position_at(&initial, "counter := coun");
        completion_pos.character += 14;
        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: completion_pos,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };

        let iterations = env_usize("ST_LSP_PERF_EDIT_LOOP_ITERS", 120).max(1);
        let avg_budget_ms = env_u64("ST_LSP_PERF_EDIT_LOOP_AVG_MS", 80);
        let p95_budget_ms = env_u64("ST_LSP_PERF_EDIT_LOOP_P95_MS", 140);
        let cpu_budget_ms = env_u64("ST_LSP_PERF_EDIT_LOOP_CPU_MS", 70);

        let cpu_start = process_cpu_millis();
        let rss_start_kb = process_rss_kb();
        let alloc_start = alloc_snapshot();
        let mut samples = Vec::with_capacity(iterations);
        let capture_breakdown = env::var_os("ST_LSP_PERF_BREAKDOWN").is_some();
        let mut update_total = Duration::from_nanos(0);
        let mut hover_total = Duration::from_nanos(0);
        let mut completion_total = Duration::from_nanos(0);

        for idx in 0..iterations {
            let next_source = edit_loop_source(idx + 1);
            let start = Instant::now();
            if capture_breakdown {
                let phase = Instant::now();
                state.update_document(&uri, (idx as i32) + 2, next_source);
                update_total += phase.elapsed();
                let phase = Instant::now();
                let _ = hover(&state, hover_params.clone());
                hover_total += phase.elapsed();
                let phase = Instant::now();
                let _ = completion(&state, completion_params.clone());
                completion_total += phase.elapsed();
            } else {
                state.update_document(&uri, (idx as i32) + 2, next_source);
                let _ = hover(&state, hover_params.clone());
                let _ = completion(&state, completion_params.clone());
            }
            samples.push(start.elapsed());
        }

        let cpu_end = process_cpu_millis();
        let rss_end_kb = process_rss_kb();
        let alloc_end = alloc_snapshot();
        let total_nanos: u128 = samples.iter().map(Duration::as_nanos).sum();
        let avg = Duration::from_nanos((total_nanos / iterations as u128) as u64);
        let p95 = percentile_duration(&samples, 95);
        let cpu_ms_per_iter = match (cpu_start, cpu_end) {
            (Some(start), Some(end)) if end >= start => (end - start) as f64 / iterations as f64,
            _ => 0.0,
        };
        let alloc_calls_delta = alloc_end
            .alloc_calls
            .saturating_sub(alloc_start.alloc_calls);
        let alloc_bytes_delta = alloc_end
            .alloc_bytes
            .saturating_sub(alloc_start.alloc_bytes);
        let dealloc_bytes_delta = alloc_end
            .dealloc_bytes
            .saturating_sub(alloc_start.dealloc_bytes);
        let alloc_calls_per_iter = alloc_calls_delta as f64 / iterations as f64;
        let alloc_bytes_per_iter = alloc_bytes_delta as f64 / iterations as f64;
        let retained_bytes = alloc_bytes_delta.saturating_sub(dealloc_bytes_delta);
        let retained_per_iter = retained_bytes as f64 / iterations as f64;
        let rss_delta_kb = match (rss_start_kb, rss_end_kb) {
            (Some(start), Some(end)) if end >= start => end - start,
            _ => 0,
        };
        let update_ms_per_iter = if capture_breakdown {
            update_total.as_secs_f64() * 1000.0 / iterations as f64
        } else {
            0.0
        };
        let hover_ms_per_iter = if capture_breakdown {
            hover_total.as_secs_f64() * 1000.0 / iterations as f64
        } else {
            0.0
        };
        let completion_ms_per_iter = if capture_breakdown {
            completion_total.as_secs_f64() * 1000.0 / iterations as f64
        } else {
            0.0
        };
        println!(
            "perf_edit_loop_budget backend=salsa avg_ms={:.2} p95_ms={:.2} cpu_ms_per_iter={:.2} update_ms_per_iter={:.2} hover_ms_per_iter={:.2} completion_ms_per_iter={:.2} alloc_calls_per_iter={:.2} alloc_bytes_per_iter={:.2} retained_bytes={:.0} retained_bytes_per_iter={:.2} rss_start_kb={} rss_end_kb={} rss_delta_kb={} iterations={}",
            avg.as_secs_f64() * 1000.0,
            p95.as_secs_f64() * 1000.0,
            cpu_ms_per_iter,
            update_ms_per_iter,
            hover_ms_per_iter,
            completion_ms_per_iter,
            alloc_calls_per_iter,
            alloc_bytes_per_iter,
            retained_bytes as f64,
            retained_per_iter,
            rss_start_kb.unwrap_or(0),
            rss_end_kb.unwrap_or(0),
            rss_delta_kb,
            iterations
        );

        assert!(
            avg.as_millis() <= avg_budget_ms as u128,
            "edit-loop avg {:?} exceeded budget {}ms",
            avg,
            avg_budget_ms
        );
        assert!(
            p95.as_millis() <= p95_budget_ms as u128,
            "edit-loop p95 {:?} exceeded budget {}ms",
            p95,
            p95_budget_ms
        );
        if cpu_start.is_some() && cpu_end.is_some() {
            assert!(
                cpu_ms_per_iter <= cpu_budget_ms as f64,
                "edit-loop CPU {:.2}ms/op exceeded budget {}ms/op",
                cpu_ms_per_iter,
                cpu_budget_ms
            );
        }
    }
}
