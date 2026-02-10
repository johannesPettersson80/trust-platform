use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use tower_lsp::lsp_types::Url;

use crate::config::ProjectConfig;
use trust_hir::SourceKey;

use super::ServerState;

pub(super) fn workspace_config_for_uri(state: &ServerState, uri: &Url) -> Option<ProjectConfig> {
    workspace_config_match_for_uri(state, uri).map(|(_, config)| config)
}

pub(super) fn workspace_config_match_for_uri(
    state: &ServerState,
    uri: &Url,
) -> Option<(Url, ProjectConfig)> {
    let configs = state.workspace_configs.read();
    if let Some(path) = uri_to_path(uri) {
        let path = normalize_match_path(path);
        let mut best: Option<(usize, Url, ProjectConfig)> = None;
        for (root_url, config) in configs.iter() {
            let Some(root_path) = uri_to_path(root_url) else {
                continue;
            };
            let root_path = normalize_match_path(root_path);
            if path.starts_with(&root_path) {
                let depth = root_path.components().count();
                update_best_match(&mut best, depth, root_url, config);
            }
        }
        if best.is_some() {
            return best.map(|(_, root, config)| (root, config));
        }
    }

    let uri_segments: Vec<_> = uri
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    let mut best: Option<(usize, Url, ProjectConfig)> = None;
    for (root_url, config) in configs.iter() {
        let root_segments: Vec<_> = root_url
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();
        if root_segments.is_empty() {
            continue;
        }
        if uri_segments.len() < root_segments.len() {
            continue;
        }
        if uri_segments[..root_segments.len()] == root_segments[..] {
            let depth = root_segments.len();
            update_best_match(&mut best, depth, root_url, config);
        }
    }
    best.map(|(_, root, config)| (root, config))
}

fn update_best_match(
    best: &mut Option<(usize, Url, ProjectConfig)>,
    depth: usize,
    root_url: &Url,
    config: &ProjectConfig,
) {
    if best
        .as_ref()
        .is_none_or(|(best_depth, _, _)| depth > *best_depth)
    {
        *best = Some((depth, root_url.clone(), config.clone()));
    }
}

#[cfg(windows)]
fn normalize_windows_path_for_url(path: &Path) -> Option<PathBuf> {
    let raw = path.to_str()?;
    if let Some(rest) = raw
        .strip_prefix("\\\\?\\UNC\\")
        .or_else(|| raw.strip_prefix("\\?\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\??\\UNC\\"))
    {
        let mut unc = String::from("\\\\");
        unc.push_str(rest);
        return Some(PathBuf::from(unc));
    }
    if let Some(rest) = raw
        .strip_prefix("\\\\?\\")
        .or_else(|| raw.strip_prefix("\\?\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\"))
        .or_else(|| raw.strip_prefix("\\??\\"))
    {
        return Some(PathBuf::from(rest));
    }
    None
}

fn path_debug_enabled() -> bool {
    std::env::var_os("TRUST_LSP_PATH_DEBUG").is_some()
}

pub(crate) fn path_to_uri(path: &Path) -> Option<Url> {
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "path_to_uri input: {}", path.display());
    }

    #[cfg(windows)]
    {
        if let Some(normalized) = normalize_windows_path_for_url(path) {
            if let Ok(url) = Url::from_file_path(&normalized) {
                if path_debug_enabled() {
                    tracing::info!(target: "trust_lsp::path", "path_to_uri output: {}", url);
                }
                return Some(url);
            }
            return path_to_uri(&normalized);
        }
    }

    if let Ok(url) = Url::from_file_path(path) {
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "path_to_uri output: {}", url);
        }
        return Some(url);
    }

    let raw_str = path.to_string_lossy();
    if !path.is_absolute() && !raw_str.starts_with('/') {
        return None;
    }
    let mut raw = raw_str.replace('\\', "/");

    #[cfg(windows)]
    {
        if raw.starts_with("//?/UNC/")
            || raw.starts_with("/?/UNC/")
            || raw.starts_with("//./UNC/")
            || raw.starts_with("/./UNC/")
            || raw.starts_with("//??/UNC/")
            || raw.starts_with("/??/UNC/")
        {
            raw = format!(
                "//{}",
                raw.trim_start_matches("//?/UNC/")
                    .trim_start_matches("/?/UNC/")
                    .trim_start_matches("//./UNC/")
                    .trim_start_matches("/./UNC/")
                    .trim_start_matches("//??/UNC/")
                    .trim_start_matches("/??/UNC/")
            );
        } else if raw.starts_with("//?/")
            || raw.starts_with("/?/")
            || raw.starts_with("//./")
            || raw.starts_with("/./")
            || raw.starts_with("//??/")
            || raw.starts_with("/??/")
        {
            raw = raw
                .trim_start_matches("//?/")
                .trim_start_matches("/?/")
                .trim_start_matches("//./")
                .trim_start_matches("/./")
                .trim_start_matches("//??/")
                .trim_start_matches("/??/")
                .to_string();
        }
    }

    if !raw.starts_with('/') {
        raw = format!("/{raw}");
    }
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "path_to_uri raw: {}", raw);
    }
    let url = Url::parse(&format!("file://{raw}")).ok();
    if let Some(ref url) = url {
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "path_to_uri output: {}", url);
        }
    }
    url
}

pub(crate) fn uri_to_path(uri: &Url) -> Option<PathBuf> {
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "uri_to_path input: {}", uri);
    }
    if let Ok(path) = uri.to_file_path() {
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "uri_to_path output: {}", path.display());
        }
        return Some(path);
    }
    if uri.scheme() == "file" {
        let raw_path = uri.path();
        if raw_path.is_empty() {
            return None;
        }
        let decoded = percent_decode_str(raw_path).decode_utf8_lossy();
        if path_debug_enabled() {
            tracing::info!(
                target: "trust_lsp::path",
                "uri_to_path decoded path: {}",
                decoded
            );
        }

        #[cfg(windows)]
        {
            if let Some(stripped) = decoded.strip_prefix('/') {
                if stripped.len() >= 2 && stripped.as_bytes()[1] == b':' {
                    return Some(PathBuf::from(stripped));
                }
            }
            if let Some(host) = uri.host_str() {
                let host = percent_decode_str(host).decode_utf8_lossy();
                let mut unc = String::from("\\\\");
                unc.push_str(&host);
                if let Some(path) = decoded.strip_prefix('/') {
                    unc.push('\\');
                    unc.push_str(&path.replace('/', "\\"));
                }
                return Some(PathBuf::from(unc));
            }
        }

        return Some(PathBuf::from(decoded.as_ref()));
    }
    None
}

pub(super) fn source_key_for_uri(uri: &Url) -> SourceKey {
    if let Some(path) = uri_to_path(uri) {
        SourceKey::from_path(path)
    } else {
        SourceKey::from_virtual(uri.to_string())
    }
}

fn normalize_match_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(windows)]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    let raw = match path.to_str() {
        Some(raw) => raw,
        None => return path,
    };

    if let Some(rest) = raw
        .strip_prefix("\\\\?\\UNC\\")
        .or_else(|| raw.strip_prefix("\\?\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\??\\UNC\\"))
    {
        let mut unc = String::from("\\\\");
        unc.push_str(rest);
        return PathBuf::from(unc);
    }

    if let Some(rest) = raw
        .strip_prefix("\\\\?\\")
        .or_else(|| raw.strip_prefix("\\?\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\"))
        .or_else(|| raw.strip_prefix("\\??\\"))
    {
        return PathBuf::from(rest);
    }

    path
}

#[cfg(not(windows))]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    path
}

pub(super) fn canonicalize_path(path: PathBuf) -> PathBuf {
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "canonicalize_path input: {}", path.display());
    }
    if let Ok(canon) = path.canonicalize() {
        let canon = strip_windows_device_prefix(canon);
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "canonicalize_path output: {}", canon.display());
        }
        return canon;
    }
    path
}
