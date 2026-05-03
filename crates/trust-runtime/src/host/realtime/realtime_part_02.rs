/// Readiness metadata for `_meta/shm_channels`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct T0ShmChannelReadiness {
    /// Logical channel identifier.
    pub channel_id: String,
    /// Stable schema identifier.
    pub schema_id: String,
    /// Schema version.
    pub schema_version: u32,
    /// Concrete schema layout hash.
    pub schema_hash: String,
    /// Slot size in bytes.
    pub slot_size: usize,
    /// Ownership contract tag.
    pub ownership: String,
    /// Stale-read threshold in missed reads.
    pub stale_after_reads: u8,
    /// Bounded seqlock retry budget.
    pub max_spin_retries: u8,
    /// Maximum bounded spin time in microseconds.
    pub max_spin_time_us: u64,
    /// Whether pages are currently pinned in RAM.
    pub pinned: bool,
    /// Whether the channel completed startup provisioning.
    pub ready: bool,
    /// Filesystem path backing this shared mapping.
    pub mapping_path: String,
}

/// Monotonic counters for stale/overrun and policy-enforcement diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct T0ChannelCounters {
    /// Overwrite-before-read events.
    pub overrun_count: u64,
    /// Stale read failures.
    pub stale_count: u64,
    /// Retry budget exhausted while writer remained unstable.
    pub spin_exhausted_count: u64,
    /// Explicitly denied T0->mesh fallback attempts.
    pub fallback_denied_count: u64,
}

/// Handle-only publisher binding for hot-path T0 publish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PubHandle {
    channel_index: usize,
    payload_size: usize,
    schema_tag: u64,
    route: RealtimeRoute,
}

/// Handle-only subscriber binding for hot-path T0 read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubHandle {
    channel_index: usize,
    payload_size: usize,
    schema_tag: u64,
    route: RealtimeRoute,
}

/// Fresh read metadata for deterministic transport diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct T0FreshRead {
    /// Bytes copied into caller buffer.
    pub bytes: usize,
    /// Monotonic write sequence observed by this read.
    pub sequence: u64,
    /// Number of updates overwritten before this read consumed fresh data.
    pub dropped_updates: u64,
    /// Aggregate channel overrun count after this read.
    pub overrun_count: u64,
}

/// Read result class for bounded deterministic consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0ReadOutcome {
    /// Fresh payload copied into caller buffer.
    Fresh(T0FreshRead),
    /// No new payload yet; stale threshold has not been crossed.
    NoUpdate,
}

/// Deterministic cycle exchange point for realtime communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T0ExchangePoint {
    /// Exchange point before task execution.
    PreTask,
    /// Exchange point after task execution.
    PostTask,
}

/// Scheduler budget policy for T0 cycle isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct T0SchedulerPolicy {
    /// Maximum noncritical/cloud-plane work units per cycle.
    pub max_cloud_ops_per_cycle: u32,
}

impl Default for T0SchedulerPolicy {
    fn default() -> Self {
        Self {
            max_cloud_ops_per_cycle: 16,
        }
    }
}

/// Deterministic cycle helper that enforces pre/post order and cloud budget isolation.
#[derive(Debug, Clone)]
pub struct T0CycleScheduler {
    policy: T0SchedulerPolicy,
    cycle: u64,
    saw_pre_task: bool,
    saw_post_task: bool,
    cloud_budget_remaining: u32,
    denied_cloud_ops_total: u64,
}

impl T0CycleScheduler {
    /// Create a scheduler with explicit policy.
    #[must_use]
    pub fn new(policy: T0SchedulerPolicy) -> Self {
        Self {
            policy,
            cycle: 0,
            saw_pre_task: false,
            saw_post_task: false,
            cloud_budget_remaining: policy.max_cloud_ops_per_cycle,
            denied_cloud_ops_total: 0,
        }
    }

    /// Start a cycle and reset deterministic exchange point state.
    pub fn begin_cycle(&mut self, cycle: u64) {
        self.cycle = cycle;
        self.saw_pre_task = false;
        self.saw_post_task = false;
        self.cloud_budget_remaining = self.policy.max_cloud_ops_per_cycle;
    }

    /// Mark an exchange point; order must be deterministic (`PreTask` -> `PostTask`).
    pub fn mark_exchange_point(&mut self, point: T0ExchangePoint) -> Result<(), T0Error> {
        match point {
            T0ExchangePoint::PreTask if self.saw_pre_task => Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "cycle {} already marked {:?}",
                    self.cycle,
                    T0ExchangePoint::PreTask
                ),
            )),
            T0ExchangePoint::PreTask => {
                self.saw_pre_task = true;
                Ok(())
            }
            T0ExchangePoint::PostTask if !self.saw_pre_task => Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "cycle {} must mark {:?} before {:?}",
                    self.cycle,
                    T0ExchangePoint::PreTask,
                    T0ExchangePoint::PostTask
                ),
            )),
            T0ExchangePoint::PostTask if self.saw_post_task => Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "cycle {} already marked {:?}",
                    self.cycle,
                    T0ExchangePoint::PostTask
                ),
            )),
            T0ExchangePoint::PostTask => {
                self.saw_post_task = true;
                Ok(())
            }
        }
    }

    /// Consume bounded noncritical/cloud-plane budget for current cycle.
    ///
    /// Returns granted work units (may be less than requested).
    pub fn consume_cloud_budget(&mut self, requested_ops: u32) -> u32 {
        let granted = requested_ops.min(self.cloud_budget_remaining);
        let denied = requested_ops.saturating_sub(granted);
        self.cloud_budget_remaining = self.cloud_budget_remaining.saturating_sub(granted);
        self.denied_cloud_ops_total = self.denied_cloud_ops_total.saturating_add(denied as u64);
        granted
    }

    /// Return total denied cloud-plane work across cycles.
    #[must_use]
    pub fn denied_cloud_ops_total(&self) -> u64 {
        self.denied_cloud_ops_total
    }

    /// Return whether current cycle has observed pre/post exchange points.
    #[must_use]
    pub fn exchange_points_seen(&self) -> (bool, bool) {
        (self.saw_pre_task, self.saw_post_task)
    }
}

fn schema_tag(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

