use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "ads.discover" => super::super::ads_handlers::handle_ads_discover(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.identity" => super::super::ads_handlers::handle_ads_identity(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.doctor" => {
            super::super::ads_handlers::handle_ads_doctor(request.id, request.params.clone(), state)
        }
        "ads.doctor.start" => super::super::ads_handlers::handle_ads_doctor_start(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.doctor.status" => super::super::ads_handlers::handle_ads_doctor_status(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.import_symbols" => super::super::ads_handlers::handle_ads_import_symbols(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.import_symbols.apply" => super::super::ads_handlers::handle_ads_import_symbols_apply(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.status" => super::super::ads_handlers::handle_ads_status(request.id, state),
        "ads.route_plan" => super::super::ads_handlers::handle_ads_route_plan(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.route_add" => super::super::ads_handlers::handle_ads_route_add(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.route_remove" => super::super::ads_handlers::handle_ads_route_remove(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.server.status" => {
            super::super::ads_handlers::handle_ads_server_status(request.id, state)
        }
        "ads.server.symbols" => {
            super::super::ads_handlers::handle_ads_server_symbols(request.id, state)
        }
        "ads.server.doctor" => super::super::ads_handlers::handle_ads_server_doctor(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.server.doctor.start" => super::super::ads_handlers::handle_ads_server_doctor_start(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.server.doctor.status" => super::super::ads_handlers::handle_ads_server_doctor_status(
            request.id,
            request.params.clone(),
            state,
        ),
        "ads.server.route_plan" => super::super::ads_handlers::handle_ads_server_route_plan(
            request.id,
            request.params.clone(),
            state,
        ),
        _ => return None,
    };
    Some(response)
}
