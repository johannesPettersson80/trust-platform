//! Config-UI route handlers for TOML/ST-first engineering workflows.

#![allow(missing_docs)]

use super::*;
use crate::harness::{CompileSession, SourceFile as HarnessSourceFile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

mod lifecycle;
mod live;
mod models;
mod responses;
mod routes;
mod runtime_cloud;
mod workspace;

use self::lifecycle::*;
use self::live::*;
use self::models::*;
use self::responses::*;
use self::runtime_cloud::*;
use self::workspace::*;

pub(super) struct ConfigUiRouteContext<'a> {
    pub mode: WebServerMode,
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub control_state: &'a Arc<ControlState>,
    pub bundle_root: &'a Option<PathBuf>,
}

pub(super) enum ConfigUiRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_config_ui_route(
    request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: ConfigUiRouteContext<'_>,
) -> ConfigUiRouteOutcome {
    if ctx.mode != WebServerMode::StandaloneIde {
        return ConfigUiRouteOutcome::NotHandled(request);
    }

    let request =
        match routes::runtime_cloud::handle_runtime_cloud_routes(request, method, url, &ctx) {
            ConfigUiRouteOutcome::Handled => return ConfigUiRouteOutcome::Handled,
            ConfigUiRouteOutcome::NotHandled(request) => request,
        };
    let request = match routes::lifecycle::handle_lifecycle_routes(request, method, url, &ctx) {
        ConfigUiRouteOutcome::Handled => return ConfigUiRouteOutcome::Handled,
        ConfigUiRouteOutcome::NotHandled(request) => request,
    };
    let request = match routes::live::handle_live_routes(request, method, url, &ctx) {
        ConfigUiRouteOutcome::Handled => return ConfigUiRouteOutcome::Handled,
        ConfigUiRouteOutcome::NotHandled(request) => request,
    };
    routes::workspace::handle_workspace_routes(request, method, url, &ctx)
}
