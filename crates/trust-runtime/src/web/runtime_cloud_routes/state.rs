use super::*;

pub(super) fn handle_get_state(request: tiny_http::Request, ctx: &RuntimeCloudRouteContext<'_>) {
    let (web_role, request_token) = match check_auth_with_role(
        &request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = request.respond(auth_error_response(error));
            return;
        }
    };
    if let Some((denial_code, denial_reason)) =
        runtime_cloud_profile_precondition(ctx.profile, ctx.auth_mode, ctx.web_tls_enabled)
    {
        let response = Response::from_string(
            json!({
                "ok": false,
                "denial_code": denial_code,
                "error": denial_reason,
                "profile": ctx.profile.as_str(),
            })
            .to_string(),
        )
        .with_status_code(StatusCode(503))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return;
    }
    let now_ns = now_ns();
    let local_runtime_text = local_runtime_id(ctx);
    let site = RUNTIME_CLOUD_DEFAULT_SITE;
    let mut peers_by_runtime = BTreeMap::<String, RuntimePresenceRecord>::new();
    for entry in ctx.discovery.snapshot() {
        let mesh_reachable = entry.mesh_port.is_some() && !entry.addresses.is_empty();
        let stale_by_age = entry.last_seen_ns > now_ns
            || now_ns.saturating_sub(entry.last_seen_ns) >= RUNTIME_CLOUD_STALE_TIMEOUT_NS;
        let web_reachable = entry.web_port.is_some() && !entry.addresses.is_empty();
        let peer_live = stale_by_age
            && web_reachable
            && runtime_cloud_peer_appears_live(&entry.addresses, entry.web_port);
        let observation = RuntimePeerObservation {
            runtime_id: entry.name.to_string(),
            site: site.to_string(),
            display_name: entry.name.to_string(),
            mesh_reachable,
            last_seen_ns: if peer_live {
                now_ns
            } else {
                entry.last_seen_ns
            },
        };
        let candidate = presence_record_from_observation(
            &observation,
            now_ns,
            PresenceThresholds {
                stale_timeout_ns: RUNTIME_CLOUD_STALE_TIMEOUT_NS,
                partition_timeout_ns: RUNTIME_CLOUD_PARTITION_TIMEOUT_NS,
            },
        );
        peers_by_runtime
            .entry(candidate.runtime_id.clone())
            .and_modify(|existing| merge_presence_record(existing, &candidate))
            .or_insert(candidate);
    }
    let peers = peers_by_runtime.into_values().collect::<Vec<_>>();
    let mut acting_on = Vec::new();
    for peer in &peers {
        if !acting_on
            .iter()
            .any(|runtime_id| runtime_id == &peer.runtime_id)
        {
            acting_on.push(peer.runtime_id.clone());
        }
    }
    if !acting_on.iter().any(|id| id == local_runtime_text.as_str()) {
        acting_on.push(local_runtime_text.clone());
    }
    let mode = if web_role.allows(AccessRole::Engineer) {
        UiMode::Edit
    } else {
        UiMode::View
    };
    let identity = if request_token.is_some() {
        format!("pairing://{}", web_role.as_str())
    } else {
        format!("local://{}", web_role.as_str())
    };
    let context = UiContext {
        connected_via: local_runtime_text.clone(),
        acting_on,
        site_scope: vec![site.to_string()],
        identity,
        role: web_role.as_str().to_string(),
        mode,
    };
    let mut state =
        project_runtime_cloud_state(context, local_runtime_text.as_str(), site, now_ns, &peers);
    runtime_cloud_apply_link_transport_preferences(
        &mut state,
        ctx.link_transport_state.as_ref(),
        Some(ctx.discovery.as_ref()),
    );
    state.topology.host_groups =
        runtime_cloud_compute_host_groups(Some(ctx.discovery.as_ref()), &state.topology.nodes);
    state.feature_flags = runtime_cloud_topology_feature_flags(ctx.profile);
    let response =
        Response::from_string(serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string()))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}

fn merge_presence_record(existing: &mut RuntimePresenceRecord, candidate: &RuntimePresenceRecord) {
    if candidate.last_seen_ns > existing.last_seen_ns {
        existing.last_seen_ns = candidate.last_seen_ns;
        existing.display_name = candidate.display_name.clone();
        existing.site = candidate.site.clone();
    }
    existing.mesh_reachable |= candidate.mesh_reachable;
    existing.stale &= candidate.stale;
    existing.partitioned &= candidate.partitioned;
}
