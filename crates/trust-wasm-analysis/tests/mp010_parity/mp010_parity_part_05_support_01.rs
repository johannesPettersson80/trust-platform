use super::*;

pub(crate) fn assert_budget(name: &str, adapter: Duration, native: Duration) {
    let adapter_us = adapter.as_micros();
    let native_us = native.as_micros();
    let ratio_limit = native_us.saturating_mul(4);
    let headroom = 120_000_u128;
    let allowed = ratio_limit.saturating_add(headroom);

    eprintln!(
        "{name}: adapter={}us native={}us allowed={}us",
        adapter_us, native_us, allowed
    );

    assert!(
        adapter_us <= allowed,
        "{name} exceeded spike budget (adapter={}us native={}us allowed={}us)",
        adapter_us,
        native_us,
        allowed
    );
    assert!(
        adapter <= Duration::from_secs(2),
        "{name} exceeded absolute 2s spike limit: {adapter:?}"
    );
}

pub(crate) fn measure_iterations<T>(iterations: usize, mut op: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(op());
    }
    start.elapsed()
}

#[cfg(target_os = "linux")]
pub(crate) fn process_memory_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let value = rest
                .split_whitespace()
                .next()
                .and_then(|text| text.parse::<u64>().ok())?;
            return Some(value);
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn process_memory_kib() -> Option<u64> {
    None
}

pub(crate) fn completion_labels_for_program_prefix(prefix: &str) -> Vec<String> {
    let mut documents = load_plant_demo_documents();
    let program_uri = "memory:///plant_demo/program.st";
    let program_index = documents
        .iter()
        .position(|doc| doc.uri == program_uri)
        .expect("program source exists");
    let anchor = "Pump(Command := Cmd);";
    let anchor_offset = documents[program_index]
        .text
        .find(anchor)
        .expect("anchor statement exists");
    let (before, after) = documents[program_index].text.split_at(anchor_offset);
    let updated_program = format!("{before}{prefix}\n{after}");
    let completion_offset = anchor_offset as u32 + prefix.len() as u32;
    documents[program_index].text = updated_program;

    let request = CompletionRequest {
        uri: program_uri.to_string(),
        position: offset_to_position_utf16(&documents[program_index].text, completion_offset),
        limit: Some(80),
    };
    let mut engine = BrowserAnalysisEngine::new();
    engine
        .replace_documents(documents)
        .expect("load plant demo documents");
    engine
        .completion(request)
        .expect("completion should succeed")
        .into_iter()
        .map(|item| item.label)
        .collect()
}

pub(crate) fn native_diagnostics(
    documents: &[DocumentInput],
    uri: &str,
) -> Vec<trust_wasm_analysis::DiagnosticItem> {
    let project = native_project(documents);
    let source = documents
        .iter()
        .find(|doc| doc.uri == uri)
        .map(|doc| doc.text.as_str())
        .expect("source exists");
    let file_id = project
        .file_id_for_key(&SourceKey::from_virtual(uri.to_string()))
        .expect("file id exists");

    let mut items = project.with_database(|db| {
        trust_ide::diagnostics::collect_diagnostics(db, file_id)
            .into_iter()
            .map(|diagnostic| {
                let mut related = diagnostic
                    .related
                    .into_iter()
                    .map(|item| RelatedInfoItem {
                        range: text_range_to_lsp(source, item.range),
                        span: trust_wasm_analysis::ByteSpan {
                            start: u32::from(item.range.start()),
                            end: u32::from(item.range.end()),
                        },
                        message: item.message,
                    })
                    .collect::<Vec<_>>();
                related.sort_by(|left, right| {
                    left.range
                        .cmp(&right.range)
                        .then_with(|| left.message.cmp(&right.message))
                });
                trust_wasm_analysis::DiagnosticItem {
                    code: diagnostic.code.code().to_string(),
                    severity: severity_label(diagnostic.severity).to_string(),
                    message: diagnostic.message,
                    range: text_range_to_lsp(source, diagnostic.range),
                    span: trust_wasm_analysis::ByteSpan {
                        start: u32::from(diagnostic.range.start()),
                        end: u32::from(diagnostic.range.end()),
                    },
                    related,
                }
            })
            .collect::<Vec<_>>()
    });
    items.sort_by(|left, right| {
        left.range
            .cmp(&right.range)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
            .then_with(|| left.severity.cmp(&right.severity))
    });
    items
}

pub(crate) fn native_hover(
    documents: &[DocumentInput],
    request: &HoverRequest,
) -> Option<HoverItem> {
    let project = native_project(documents);
    let source = documents
        .iter()
        .find(|doc| doc.uri == request.uri)
        .map(|doc| doc.text.as_str())
        .expect("source exists");
    let file_id = project
        .file_id_for_key(&SourceKey::from_virtual(request.uri.clone()))
        .expect("file id exists");
    let offset = position_to_offset_utf16(source, request.position.clone()).expect("offset");

    project.with_database(|db| {
        trust_ide::hover_with_filter(
            db,
            file_id,
            TextSize::from(offset),
            &StdlibFilter::allow_all(),
        )
        .map(|hover| HoverItem {
            contents: hover.contents,
            range: hover.range.map(|range| text_range_to_lsp(source, range)),
        })
    })
}
