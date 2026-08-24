fn run_dispatch_bench(workload: DispatchBenchWorkload) -> anyhow::Result<BenchReport> {
    let local_runtime = "runtime-a";
    let mut target_status = BTreeMap::new();
    for idx in 0..workload.fanout {
        target_status.insert(
            format!("runtime-{}", idx + 1),
            RuntimeCloudTargetStatus {
                reachable: true,
                stale: false,
                supports_secure_transport: true,
            },
        );
    }

    let target_ids = target_status.keys().cloned().collect::<Vec<_>>();
    let mut preflight_ns = Vec::with_capacity(workload.base.samples);
    let mut dispatch_ns = Vec::with_capacity(workload.base.samples);
    let mut end_to_end_ns = Vec::with_capacity(workload.base.samples);
    let mut audit_correlation_ns = Vec::with_capacity(workload.base.samples);

    for idx in 0..workload.base.samples {
        let mut payload = vec![0_u8; workload.base.payload_bytes];
        let stamp = (idx as u64).to_le_bytes();
        for (slot, byte) in payload.iter_mut().enumerate() {
            *byte = stamp[slot % stamp.len()];
        }

        let action = RuntimeCloudActionRequest {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            request_id: format!("bench-dispatch-{idx}"),
            connected_via: local_runtime.to_string(),
            target_runtimes: target_ids.clone(),
            actor: "bench-operator".to_string(),
            action_type: "cmd_invoke".to_string(),
            query_budget_ms: Some(1_000),
            dry_run: false,
            payload: json!({
                "command": "status",
                "params": {"blob": payload},
            }),
        };

        let e2e_started = Instant::now();
        let preflight_started = Instant::now();
        let preflight = preflight_action(
            &action,
            RuntimeCloudPreflightContext {
                local_runtime_id: local_runtime,
                role: AccessRole::Engineer,
            },
            &target_status,
        );
        preflight_ns.push(duration_ns(preflight_started));

        if !preflight.allowed {
            anyhow::bail!(
                "dispatch benchmark preflight denied: {}",
                preflight
                    .denial_reason
                    .unwrap_or_else(|| "unknown denial".to_string())
            );
        }

        let dispatch_started = Instant::now();
        let control_request = map_action_to_control_request(&action)
            .map_err(|(_, reason)| anyhow::anyhow!("dispatch mapping failed: {reason}"))?;

        for decision in &preflight.decisions {
            let response = json!({
                "ok": true,
                "request_id": action.request_id,
                "target": decision.runtime_id,
                "audit_id": format!("audit-{}-{}", idx, decision.runtime_id),
                "control": control_request,
            });
            let audit_started = Instant::now();
            let wire = serde_json::to_vec(&response).context("encode dispatch response")?;
            let decoded: serde_json::Value =
                serde_json::from_slice(&wire).context("decode dispatch response")?;
            let _audit_id = decoded
                .get("audit_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("dispatch response missing audit_id"))?;
            audit_correlation_ns.push(duration_ns(audit_started));
        }

        dispatch_ns.push(duration_ns(dispatch_started));
        end_to_end_ns.push(duration_ns(e2e_started));
    }

    let report = DispatchBenchReport {
        scenario: "dispatch",
        payload_bytes: workload.base.payload_bytes,
        fanout: workload.fanout,
        preflight_latency: summarize_ns(preflight_ns.as_slice()),
        dispatch_latency: summarize_ns(dispatch_ns.as_slice()),
        end_to_end_latency: summarize_ns(end_to_end_ns.as_slice()),
        audit_correlation_latency: summarize_ns(audit_correlation_ns.as_slice()),
        histogram: histogram_from_ns(end_to_end_ns.as_slice()),
    };
    Ok(BenchReport::Dispatch(report))
}
