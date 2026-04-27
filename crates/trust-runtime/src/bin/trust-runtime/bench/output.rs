fn render_bench_output(report: &BenchReport, format: BenchOutputFormat) -> anyhow::Result<String> {
    match format {
        BenchOutputFormat::Json => {
            let mut text = serde_json::to_string_pretty(report).context("encode bench json")?;
            text.push('\n');
            Ok(text)
        }
        BenchOutputFormat::Table => Ok(render_table(report)),
    }
}

fn render_table(report: &BenchReport) -> String {
    let mut out = String::new();
    match report {
        BenchReport::Project(data) => {
            let _ = writeln!(out, "Benchmark: {}", data.scenario);
            let _ = writeln!(out, "project={}", data.project);
            let _ = writeln!(
                out,
                "resource={} cycle_budget={:.3}us samples={} warmup_cycles={} total_cycles={}",
                data.resource_name,
                data.cycle_budget_us,
                data.samples,
                data.warmup_cycles,
                data.total_cycles
            );
            render_latency_block(&mut out, "cycle latency", &data.cycle_latency);
            let _ = writeln!(
                out,
                "throughput={:.3} cycles/sec measured_duration_ms={:.3}",
                data.throughput_cycles_per_sec, data.measured_duration_ms
            );
            let _ = writeln!(out, "budget_overruns={}", data.budget_overruns);
            if !data.watched_globals.is_empty() {
                let _ = writeln!(out, "watched globals:");
                for (name, value) in &data.watched_globals {
                    let _ = writeln!(out, "  {} = {}", name, value);
                }
            }
            if let Some(vm_profile) = &data.vm_profile {
                let _ = writeln!(
                    out,
                    "vm profile: executed={} fallbacks={}",
                    vm_profile.register_programs_executed, vm_profile.register_program_fallbacks
                );
                if !vm_profile.fallback_reasons.is_empty() {
                    let _ = writeln!(out, "  fallback reasons:");
                    for reason in vm_profile.fallback_reasons.iter().take(5) {
                        let _ = writeln!(out, "    {} = {}", reason.reason, reason.count);
                    }
                }
                if !vm_profile.hot_blocks.is_empty() {
                    let _ = writeln!(out, "  hot blocks:");
                    for block in vm_profile.hot_blocks.iter().take(5) {
                        let pou = block.pou_name.as_deref().unwrap_or("<unknown>");
                        let _ = writeln!(
                            out,
                            "    pou={} ({}) block={} pc={} hits={}",
                            block.pou_id, pou, block.block_id, block.start_pc, block.hits
                        );
                    }
                }
                let ref_ops = &vm_profile.ref_ops;
                let _ = writeln!(
                    out,
                    "  ref-ops: load_ref={} store_ref={} load_ref_addr={} ref_field={} ref_index={} load_dynamic={} store_dynamic={} instance_field_lookups={}",
                    ref_ops.load_ref,
                    ref_ops.store_ref,
                    ref_ops.load_ref_addr,
                    ref_ops.ref_field,
                    ref_ops.ref_index,
                    ref_ops.load_dynamic,
                    ref_ops.store_dynamic,
                    ref_ops.instance_field_lookups
                );
                let call_ops = &vm_profile.call_ops;
                let _ = writeln!(
                    out,
                    "  call-ops: frame_pushes={} frame_pops={} function_block_call_entries={} parameter_bindings={} output_copy_backs={}",
                    call_ops.frame_pushes,
                    call_ops.frame_pops,
                    call_ops.function_block_call_entries,
                    call_ops.parameter_bindings,
                    call_ops.output_copy_backs
                );
                let value_ops = &vm_profile.value_ops;
                let _ = writeln!(
                    out,
                    "  value-ops: const_load_clones={} register_read_clones={} register_read_moves={} read_value_clones={} binding_expr_clones={} output_value_clones={}",
                    value_ops.const_load_clones,
                    value_ops.register_read_clones,
                    value_ops.register_read_moves,
                    value_ops.read_value_clones,
                    value_ops.binding_expr_clones,
                    value_ops.output_value_clones
                );
                let lowering_cache = &vm_profile.register_lowering_cache;
                let _ = writeln!(
                    out,
                    "  register-lowering-cache: enabled={} cache={}/{} hits={} misses={} hit_ratio={:.4} build_errors={} evictions={} invalidations={}",
                    lowering_cache.enabled,
                    lowering_cache.cached_entries,
                    lowering_cache.cache_capacity,
                    lowering_cache.hits,
                    lowering_cache.misses,
                    lowering_cache.hit_ratio,
                    lowering_cache.build_errors,
                    lowering_cache.cache_evictions,
                    lowering_cache.invalidations
                );
                if let Some(tier1) = &vm_profile.tier1_specialized_executor {
                    let _ = writeln!(
                        out,
                        "  tier1-specialized-executor: enabled={} threshold={} cache={}/{} compile={}/{}/{} evictions={} executions={} deopts={}",
                        tier1.enabled,
                        tier1.hot_block_threshold,
                        tier1.cached_blocks,
                        tier1.cache_capacity,
                        tier1.compile_attempts,
                        tier1.compile_successes,
                        tier1.compile_failures,
                        tier1.cache_evictions,
                        tier1.block_executions,
                        tier1.deopt_count
                    );
                    if !tier1.compile_failure_reasons.is_empty() {
                        let reasons = tier1
                            .compile_failure_reasons
                            .iter()
                            .map(|entry| format!("{}={}", entry.reason, entry.count))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let _ = writeln!(out, "    compile-failure-reasons: {reasons}");
                    }
                    if !tier1.deopt_reasons.is_empty() {
                        let reasons = tier1
                            .deopt_reasons
                            .iter()
                            .map(|entry| format!("{}={}", entry.reason, entry.count))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let _ = writeln!(out, "    deopt-reasons: {reasons}");
                    }
                }
            }
            render_histogram(&mut out, data.histogram.as_slice());
        }
        BenchReport::Init(data) => {
            let _ = writeln!(out, "Benchmark: {}", data.scenario);
            let _ = writeln!(out, "project={}", data.project);
            let _ = writeln!(
                out,
                "resource={} backend={} samples={} warmup_cycles={}",
                data.resource_name, data.execution_backend, data.samples, data.warmup_cycles
            );
            render_latency_block(&mut out, "init-only latency", &data.init_only_latency);
            render_latency_block(
                &mut out,
                "init+first-cycle latency",
                &data.init_plus_first_cycle_latency,
            );
            render_latency_block(&mut out, "first-cycle latency", &data.first_cycle_latency);
            render_latency_block(
                &mut out,
                "first-mutation latency",
                &data.first_mutation_latency,
            );
            render_latency_block(
                &mut out,
                "retain-restart latency",
                &data.retain_restart_latency,
            );
            render_latency_block(
                &mut out,
                "StructValue::new latency",
                &data.struct_value_new_latency,
            );
            render_latency_block(
                &mut out,
                "StructValue::from_untyped_parts latency",
                &data.struct_value_untyped_latency,
            );
            render_latency_block(&mut out, "steady-cycle latency", &data.steady_cycle_latency);
            render_histogram(&mut out, data.histogram.as_slice());
        }
        BenchReport::T0Shm(data) => {
            let _ = writeln!(out, "Benchmark: {}", data.scenario);
            render_latency_block(&mut out, "one-way latency", &data.one_way_latency);
            render_latency_block(&mut out, "round-trip latency", &data.round_trip_latency);
            render_latency_block(&mut out, "jitter", &data.jitter);
            let _ = writeln!(
                out,
                "overruns={} stale_reads={} spin_exhausted={} fallback_denied={}",
                data.overruns, data.stale_reads, data.spin_exhausted, data.fallback_denied
            );
            render_histogram(&mut out, data.histogram.as_slice());
        }
        BenchReport::MeshZenoh(data) => {
            let _ = writeln!(out, "Benchmark: {}", data.scenario);
            render_latency_block(&mut out, "pub/sub latency", &data.pub_sub_latency);
            render_latency_block(&mut out, "pub/sub jitter", &data.pub_sub_jitter);
            render_latency_block(&mut out, "query/reply latency", &data.query_reply_latency);
            let _ = writeln!(
                out,
                "loss_count={} reorder_count={} configured_loss_rate={:.3} configured_reorder_rate={:.3}",
                data.loss_count,
                data.reorder_count,
                data.configured_loss_rate,
                data.configured_reorder_rate
            );
            render_histogram(&mut out, data.histogram.as_slice());
        }
        BenchReport::Dispatch(data) => {
            let _ = writeln!(out, "Benchmark: {}", data.scenario);
            let _ = writeln!(out, "fanout={}", data.fanout);
            render_latency_block(&mut out, "preflight latency", &data.preflight_latency);
            render_latency_block(&mut out, "dispatch latency", &data.dispatch_latency);
            render_latency_block(&mut out, "end-to-end latency", &data.end_to_end_latency);
            render_latency_block(
                &mut out,
                "audit-correlation latency",
                &data.audit_correlation_latency,
            );
            render_histogram(&mut out, data.histogram.as_slice());
        }
    }
    out
}

fn render_latency_block(out: &mut String, label: &str, summary: &LatencySummary) {
    let _ = writeln!(
        out,
        "{label}: samples={} min={:.3}us p50={:.3}us p95={:.3}us p99={:.3}us max={:.3}us",
        summary.samples,
        summary.min_us,
        summary.p50_us,
        summary.p95_us,
        summary.p99_us,
        summary.max_us
    );
}

fn render_histogram(out: &mut String, buckets: &[HistogramBucket]) {
    let _ = writeln!(out, "histogram:");
    for bucket in buckets {
        match bucket.upper_us {
            Some(upper) => {
                let _ = writeln!(out, "  <= {:>6}us : {}", upper, bucket.count);
            }
            None => {
                let _ = writeln!(
                    out,
                    "  >  {:>6}us : {}",
                    HISTOGRAM_LIMITS_US[HISTOGRAM_LIMITS_US.len() - 1],
                    bucket.count
                );
            }
        }
    }
}
