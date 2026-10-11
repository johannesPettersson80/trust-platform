use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};
use tower_lsp::lsp_types::{InlineValue, InlineValueParams, InlineValueText, Range};
use tracing::{debug, warn};

use crate::handlers::runtime_values::{fetch_runtime_inline_values, RuntimeInlineValues};
use crate::state::ServerState;
use trust_ide::{inline_value_data, InlineValueScope};

use super::super::config::{bool_with_aliases, lsp_runtime_section, string_with_aliases};
use super::super::lsp_utils::{offset_to_position, position_to_offset};

fn runtime_inline_values_enabled(state: &ServerState) -> bool {
    let value = state.config();
    let Some(runtime) = lsp_runtime_section(&value) else {
        return true;
    };
    bool_with_aliases(runtime, &["inlineValuesEnabled", "inline_values_enabled"]).unwrap_or(true)
}

fn runtime_control_override(state: &ServerState) -> (Option<String>, Option<String>) {
    let value = state.config();
    let runtime = match lsp_runtime_section(&value) {
        Some(runtime) => runtime,
        None => return (None, None),
    };
    let control_enabled = bool_with_aliases(
        runtime,
        &["controlEndpointEnabled", "control_endpoint_enabled"],
    )
    .unwrap_or(true);
    if !control_enabled {
        return (None, None);
    }
    let endpoint = string_with_aliases(runtime, &["controlEndpoint", "control_endpoint"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let auth = string_with_aliases(runtime, &["controlAuthToken", "control_auth_token"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    (endpoint, auth)
}

pub fn inline_value(state: &ServerState, params: InlineValueParams) -> Option<Vec<InlineValue>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let start_offset = position_to_offset(&doc.content, params.range.start)?;
    let end_offset = position_to_offset(&doc.content, params.range.end)?;
    if end_offset < start_offset {
        return Some(Vec::new());
    }

    if !runtime_inline_values_enabled(state) {
        debug!("inlineValue skipped: disabled via settings");
        return Some(Vec::new());
    }

    let data = state.with_database(|db| {
        inline_value_data(
            db,
            doc.file_id,
            TextRange::new(TextSize::from(start_offset), TextSize::from(end_offset)),
        )
    });
    debug!(
        "inlineValue request uri={} frame_id={} range=({},{})->({},{}) targets={} hints={}",
        uri,
        params.context.frame_id,
        params.range.start.line,
        params.range.start.character,
        params.range.end.line,
        params.range.end.character,
        data.targets.len(),
        data.hints.len()
    );

    let mut values = Vec::new();
    let mut seen = FxHashSet::default();

    for hint in data.hints {
        seen.insert(hint.range);
        values.push(InlineValue::Text(InlineValueText {
            range: text_range_to_lsp(&doc.content, hint.range),
            text: hint.text,
        }));
    }

    let frame_id = u32::try_from(params.context.frame_id).ok();
    let mut owner_hints = Vec::new();
    for target in &data.targets {
        if let Some(owner) = target.owner.as_ref()
            && !owner_hints
                .iter()
                .any(|name: &SmolStr| name.eq_ignore_ascii_case(owner))
        {
            owner_hints.push(owner.clone());
        }
    }
    let (override_endpoint, override_auth) = runtime_control_override(state);
    let config = state.workspace_config_for_uri(uri);
    let endpoint = config
        .as_ref()
        .and_then(|config| config.runtime.control_endpoint.as_deref())
        .or(override_endpoint.as_deref());
    let auth = config
        .as_ref()
        .and_then(|config| config.runtime.control_auth_token.as_deref())
        .or(override_auth.as_deref());
    if let (Some(frame_id), Some(endpoint)) = (frame_id, endpoint) {
        debug!(
            "inlineValue runtime fetch uri={} endpoint={} auth_present={} owner_hints={}",
            uri,
            endpoint,
            auth.is_some(),
            owner_hints.len()
        );
        if let Some(runtime_values) =
            fetch_runtime_inline_values(endpoint, auth, frame_id, &owner_hints)
        {
            debug!(
                "inlineValue runtime values locals={} globals={} retain={}",
                runtime_values.locals.len(),
                runtime_values.globals.len(),
                runtime_values.retain.len()
            );
            let normalized_values = NormalizedInlineValues::new(&runtime_values);
            for target in data.targets {
                if seen.contains(&target.range) {
                    continue;
                }
                let value = normalized_values.lookup(target.scope, &target.name);
                if let Some(value) = value {
                    seen.insert(target.range);
                    values.push(InlineValue::Text(InlineValueText {
                        range: text_range_to_lsp(&doc.content, target.range),
                        text: format!(" = {value}"),
                    }));
                }
            }
        }
    } else if frame_id.is_none() {
        warn!(
            "inlineValue skipped: invalid frame_id={} for uri={}",
            params.context.frame_id, uri
        );
    } else {
        warn!(
            "inlineValue skipped: missing runtime control endpoint for uri={}",
            uri
        );
    }

    Some(values)
}

struct NormalizedInlineValues {
    locals: FxHashMap<SmolStr, String>,
    globals: FxHashMap<SmolStr, String>,
    retain: FxHashMap<SmolStr, String>,
}

impl NormalizedInlineValues {
    fn new(values: &RuntimeInlineValues) -> Self {
        Self {
            locals: normalize_inline_values(&values.locals),
            globals: normalize_inline_values(&values.globals),
            retain: normalize_inline_values(&values.retain),
        }
    }

    fn lookup(&self, scope: InlineValueScope, name: &SmolStr) -> Option<&String> {
        match scope {
            InlineValueScope::Local => lookup_inline_value(&self.locals, name)
                .or_else(|| lookup_inline_value(&self.globals, name))
                .or_else(|| lookup_inline_value(&self.retain, name)),
            InlineValueScope::Global => lookup_inline_value(&self.globals, name),
            InlineValueScope::Retain => lookup_inline_value(&self.retain, name),
        }
    }
}

fn normalize_inline_values(values: &FxHashMap<SmolStr, String>) -> FxHashMap<SmolStr, String> {
    let mut out = FxHashMap::default();
    for (name, value) in values {
        out.insert(normalize_inline_name(name), value.clone());
    }
    out
}

fn lookup_inline_value<'a>(
    values: &'a FxHashMap<SmolStr, String>,
    name: &SmolStr,
) -> Option<&'a String> {
    values
        .get(name)
        .or_else(|| values.get(&normalize_inline_name(name)))
}

fn normalize_inline_name(name: &SmolStr) -> SmolStr {
    SmolStr::new(name.as_str().to_ascii_uppercase())
}

fn text_range_to_lsp(source: &str, range: TextRange) -> Range {
    Range {
        start: offset_to_position(source, range.start().into()),
        end: offset_to_position(source, range.end().into()),
    }
}
