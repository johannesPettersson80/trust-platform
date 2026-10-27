//! Program/source + IDE HTTP route handlers for web server.

#![allow(missing_docs)]

use super::*;

mod analysis;
mod analysis_language;
mod basic;
mod files;
mod settings;
mod tasks;

pub(super) struct IdeRouteContext<'a> {
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub control_state: &'a Arc<ControlState>,
    pub bundle_root: &'a Option<PathBuf>,
    pub ide_state: &'a Arc<WebIdeState>,
    pub ide_task_store: &'a Arc<Mutex<HashMap<u64, IdeTaskJob>>>,
    pub ide_task_seq: &'a Arc<AtomicU64>,
}

pub(super) enum IdeRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_ide_route(
    request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: IdeRouteContext<'_>,
) -> IdeRouteOutcome {
    let request = match basic::handle_basic_route(request, method, url, &ctx) {
        IdeRouteOutcome::Handled => return IdeRouteOutcome::Handled,
        IdeRouteOutcome::NotHandled(request) => request,
    };
    let request = match files::handle_file_route(request, method, url, &ctx) {
        IdeRouteOutcome::Handled => return IdeRouteOutcome::Handled,
        IdeRouteOutcome::NotHandled(request) => request,
    };
    let request = match analysis::handle_analysis_route(request, method, url, &ctx) {
        IdeRouteOutcome::Handled => return IdeRouteOutcome::Handled,
        IdeRouteOutcome::NotHandled(request) => request,
    };
    let request = match settings::handle_settings_route(request, method, url, &ctx) {
        IdeRouteOutcome::Handled => return IdeRouteOutcome::Handled,
        IdeRouteOutcome::NotHandled(request) => request,
    };
    tasks::handle_task_route(request, method, url, &ctx)
}
