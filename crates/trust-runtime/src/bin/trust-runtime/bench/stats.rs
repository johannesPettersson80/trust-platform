const HISTOGRAM_LIMITS_US: &[u64] = &[5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000];

fn duration_ns(start: Instant) -> u64 {
    start
        .elapsed()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn ns_to_us(value: u64) -> f64 {
    (value as f64) / 1_000.0
}

fn quantile_index(len: usize, quantile: f64) -> usize {
    if len == 0 {
        return 0;
    }
    let rank = (quantile * len as f64).ceil() as usize;
    rank.saturating_sub(1).min(len - 1)
}

fn summarize_ns(samples_ns: &[u64]) -> LatencySummary {
    if samples_ns.is_empty() {
        return LatencySummary {
            samples: 0,
            min_us: 0.0,
            p50_us: 0.0,
            p95_us: 0.0,
            p99_us: 0.0,
            max_us: 0.0,
        };
    }

    let mut sorted = samples_ns.to_vec();
    sorted.sort_unstable();
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let p50 = sorted[quantile_index(sorted.len(), 0.50)];
    let p95 = sorted[quantile_index(sorted.len(), 0.95)];
    let p99 = sorted[quantile_index(sorted.len(), 0.99)];

    LatencySummary {
        samples: sorted.len(),
        min_us: ns_to_us(min),
        p50_us: ns_to_us(p50),
        p95_us: ns_to_us(p95),
        p99_us: ns_to_us(p99),
        max_us: ns_to_us(max),
    }
}

fn jitter_samples_ns(samples_ns: &[u64]) -> Vec<u64> {
    if samples_ns.len() < 2 {
        return Vec::new();
    }
    let mut jitter = Vec::with_capacity(samples_ns.len() - 1);
    let mut last = samples_ns[0];
    for current in &samples_ns[1..] {
        let delta = if *current >= last {
            current.saturating_sub(last)
        } else {
            last.saturating_sub(*current)
        };
        jitter.push(delta);
        last = *current;
    }
    jitter
}

fn histogram_from_ns(samples_ns: &[u64]) -> Vec<HistogramBucket> {
    let mut counts = vec![0_u64; HISTOGRAM_LIMITS_US.len() + 1];
    for sample in samples_ns {
        let sample_us = sample.saturating_add(999) / 1_000;
        let mut placed = false;
        for (idx, upper) in HISTOGRAM_LIMITS_US.iter().enumerate() {
            if sample_us <= *upper {
                counts[idx] = counts[idx].saturating_add(1);
                placed = true;
                break;
            }
        }
        if !placed {
            let last = counts.len() - 1;
            counts[last] = counts[last].saturating_add(1);
        }
    }

    let mut buckets = Vec::with_capacity(counts.len());
    for (idx, count) in counts.into_iter().enumerate() {
        let upper_us = HISTOGRAM_LIMITS_US.get(idx).copied();
        buckets.push(HistogramBucket { upper_us, count });
    }
    buckets
}

#[derive(Debug, Clone)]
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        let initial = if seed == 0 { 0xD00D_F00D_BAAD_F00D } else { seed };
        Self { state: initial }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        (bits as f64) / ((1_u64 << 53) as f64)
    }
}
