#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct SyntheticMeshEnvelope {
    sequence: u64,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct SyntheticQuery {
    id: u64,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct SyntheticReply {
    id: u64,
    accepted: bool,
    audit_id: String,
}

#[derive(Debug)]
struct PendingMeshPacket {
    sequence: u64,
    sent_at: Instant,
    encoded: Vec<u8>,
}

fn run_mesh_zenoh_bench(workload: MeshBenchWorkload) -> anyhow::Result<BenchReport> {
    let mut rng = Lcg::new(0x5EED_5EED_1234_9876);
    let mut queue = VecDeque::<PendingMeshPacket>::new();
    let mut expected_sequence = 0_u64;

    let mut pub_sub_latency_ns = Vec::with_capacity(workload.base.samples);
    let mut query_reply_latency_ns = Vec::with_capacity(workload.base.samples);
    let mut loss_count = 0_u64;
    let mut reorder_count = 0_u64;

    for idx in 0..workload.base.samples {
        let mut payload = vec![0_u8; workload.base.payload_bytes];
        let stamp = (idx as u64).to_le_bytes();
        for (slot, byte) in payload.iter_mut().enumerate() {
            *byte = stamp[slot % stamp.len()];
        }

        let sequence = idx as u64;
        let envelope = SyntheticMeshEnvelope { sequence, payload };
        let encoded = serde_json::to_vec(&envelope).context("encode mesh envelope")?;

        if rng.next_f64() < workload.loss_rate {
            loss_count = loss_count.saturating_add(1);
        } else {
            queue.push_back(PendingMeshPacket {
                sequence,
                sent_at: Instant::now(),
                encoded,
            });
        }

        if queue.len() > 1 && rng.next_f64() < workload.reorder_rate {
            if let Some(last) = queue.pop_back() {
                queue.push_front(last);
                reorder_count = reorder_count.saturating_add(1);
            }
        }

        if idx % 2 == 0 || queue.len() > 8 {
            drain_mesh_packet(
                &mut queue,
                &mut pub_sub_latency_ns,
                &mut expected_sequence,
                &mut reorder_count,
            )?;
        }

        let query_started = Instant::now();
        let query = SyntheticQuery {
            id: idx as u64,
            payload: vec![0xAB_u8; workload.base.payload_bytes],
        };
        let query_wire = serde_json::to_vec(&query).context("encode synthetic query")?;
        let decoded_query: SyntheticQuery =
            serde_json::from_slice(&query_wire).context("decode synthetic query")?;
        let reply = SyntheticReply {
            id: decoded_query.id,
            accepted: true,
            audit_id: format!("audit-mesh-{idx}"),
        };
        let reply_wire = serde_json::to_vec(&reply).context("encode synthetic reply")?;
        let _: SyntheticReply =
            serde_json::from_slice(&reply_wire).context("decode synthetic reply")?;
        query_reply_latency_ns.push(duration_ns(query_started));
    }

    while !queue.is_empty() {
        drain_mesh_packet(
            &mut queue,
            &mut pub_sub_latency_ns,
            &mut expected_sequence,
            &mut reorder_count,
        )?;
    }

    let report = MeshZenohBenchReport {
        scenario: "mesh-zenoh",
        payload_bytes: workload.base.payload_bytes,
        query_payload_bytes: workload.base.payload_bytes,
        pub_sub_latency: summarize_ns(pub_sub_latency_ns.as_slice()),
        pub_sub_jitter: summarize_ns(jitter_samples_ns(pub_sub_latency_ns.as_slice()).as_slice()),
        query_reply_latency: summarize_ns(query_reply_latency_ns.as_slice()),
        histogram: histogram_from_ns(pub_sub_latency_ns.as_slice()),
        loss_count,
        reorder_count,
        configured_loss_rate: workload.loss_rate,
        configured_reorder_rate: workload.reorder_rate,
    };
    Ok(BenchReport::MeshZenoh(report))
}

fn drain_mesh_packet(
    queue: &mut VecDeque<PendingMeshPacket>,
    pub_sub_latency_ns: &mut Vec<u64>,
    expected_sequence: &mut u64,
    reorder_count: &mut u64,
) -> anyhow::Result<()> {
    let Some(packet) = queue.pop_front() else {
        return Ok(());
    };
    let decoded: SyntheticMeshEnvelope =
        serde_json::from_slice(packet.encoded.as_slice()).context("decode mesh envelope")?;
    if decoded.sequence != packet.sequence {
        anyhow::bail!(
            "mesh packet corruption: encoded sequence {} != expected {}",
            decoded.sequence,
            packet.sequence
        );
    }
    if decoded.sequence != *expected_sequence {
        *reorder_count = reorder_count.saturating_add(1);
    }
    if decoded.sequence >= *expected_sequence {
        *expected_sequence = decoded.sequence.saturating_add(1);
    }
    pub_sub_latency_ns.push(duration_ns(packet.sent_at));
    Ok(())
}
