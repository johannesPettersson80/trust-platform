fn run_t0_shm_bench(workload: BenchWorkload) -> anyhow::Result<BenchReport> {
    let mut transport = T0Transport::new();
    let slot_size = workload.payload_bytes.saturating_mul(2).max(32);
    transport.register_channel(
        "bench-t0",
        "sha256:bench-t0",
        T0ChannelPolicy {
            slot_size,
            stale_after_reads: 3,
            max_spin_retries: 4,
            max_spin_time_us: 100,
        },
    )?;

    let pub_handle = transport.bind_publisher(
        "bench-t0",
        RealtimeRoute::T0HardRt,
        "sha256:bench-t0",
        workload.payload_bytes,
        true,
    )?;
    let sub_handle = transport.bind_subscriber(
        "bench-t0",
        RealtimeRoute::T0HardRt,
        "sha256:bench-t0",
        workload.payload_bytes,
        true,
    )?;

    let mut one_way_ns = Vec::with_capacity(workload.samples);
    let mut round_trip_ns = Vec::with_capacity(workload.samples);
    let mut out = vec![0_u8; workload.payload_bytes];

    for idx in 0..workload.samples {
        let mut payload = vec![0_u8; workload.payload_bytes];
        let stamp = (idx as u64).to_le_bytes();
        for (slot, byte) in payload.iter_mut().enumerate() {
            *byte = stamp[slot % stamp.len()];
        }

        let publish_started = Instant::now();
        transport.publish_hardrt(pub_handle, payload.as_slice())?;
        let publish_elapsed = duration_ns(publish_started);
        one_way_ns.push(publish_elapsed);

        // Force periodic overwrite-before-read to keep overrun accounting visible.
        if idx % 8 == 0 {
            transport.publish_hardrt(pub_handle, payload.as_slice())?;
        }

        let round_started = Instant::now();
        loop {
            match transport.read_hardrt(sub_handle, out.as_mut_slice()) {
                Ok(T0ReadOutcome::Fresh(_)) => break,
                Ok(T0ReadOutcome::NoUpdate) => std::hint::spin_loop(),
                Err(error) => anyhow::bail!("t0 read failed during benchmark: {error}"),
            }
        }
        round_trip_ns.push(duration_ns(round_started));
    }

    let counters = transport.channel_counters("bench-t0").unwrap_or(T0ChannelCounters {
        overrun_count: 0,
        stale_count: 0,
        spin_exhausted_count: 0,
        fallback_denied_count: 0,
    });

    let report = T0ShmBenchReport {
        scenario: "t0-shm",
        payload_bytes: workload.payload_bytes,
        one_way_latency: summarize_ns(one_way_ns.as_slice()),
        round_trip_latency: summarize_ns(round_trip_ns.as_slice()),
        jitter: summarize_ns(jitter_samples_ns(round_trip_ns.as_slice()).as_slice()),
        histogram: histogram_from_ns(round_trip_ns.as_slice()),
        overruns: counters.overrun_count,
        stale_reads: counters.stale_count,
        spin_exhausted: counters.spin_exhausted_count,
        fallback_denied: transport.fallback_denied_total(),
    };
    Ok(BenchReport::T0Shm(report))
}
