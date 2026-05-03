use std::time::{Duration, Instant};

use super::shm::{ShmChannel, ShmChannelContract};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum T0BindMode {
    Publisher,
    Subscriber,
}

#[derive(Debug)]
struct T0ChannelState {
    id: String,
    schema_id: String,
    schema_version: u32,
    schema_hash: String,
    schema_tag: u64,
    policy: T0ChannelPolicy,
    ownership: T0ChannelOwnership,
    stale_misses: u8,
    force_unpinned_for_test: bool,
    ready: bool,
    publisher_bound: bool,
    subscriber_bound: bool,
    shm: ShmChannel,
    counters: T0ChannelCounters,
}

impl T0ChannelState {
    fn new(spec: T0ChannelSpec, config: &T0ShmConfig) -> Result<Self, T0Error> {
        let channel_tag = schema_tag(spec.channel_id.as_str());
        let schema_id_tag = schema_tag(spec.schema_id.as_str());
        let schema_hash_tag = schema_tag(spec.schema_hash.as_str());
        let contract = ShmChannelContract {
            channel_tag,
            schema_id_tag,
            schema_hash_tag,
            schema_version: spec.schema_version,
            slot_size: spec.policy.slot_size,
            stale_after_reads: spec.policy.stale_after_reads,
            max_spin_retries: spec.policy.max_spin_retries,
            max_spin_time_us: spec.policy.max_spin_time_us,
            ownership: spec.ownership,
            channel_id: spec.channel_id.as_str(),
        };
        let shm = ShmChannel::create_or_open(config, contract).map_err(|message| {
            T0Error::new(
                T0ErrorCode::TransportFailure,
                format!(
                    "failed to provision SHM for '{}': {message}",
                    spec.channel_id
                ),
            )
        })?;

        Ok(Self {
            id: spec.channel_id,
            schema_id: spec.schema_id,
            schema_version: spec.schema_version,
            schema_tag: schema_hash_tag,
            schema_hash: spec.schema_hash,
            policy: spec.policy,
            ownership: spec.ownership,
            stale_misses: 0,
            force_unpinned_for_test: false,
            ready: true,
            publisher_bound: false,
            subscriber_bound: false,
            shm,
            counters: T0ChannelCounters::default(),
        })
    }

    #[must_use]
    fn is_pinned(&self) -> bool {
        self.shm.is_pinned() && !self.force_unpinned_for_test
    }

    #[must_use]
    fn is_ready_for_io(&self) -> bool {
        self.ready && self.is_pinned()
    }

    #[must_use]
    fn readiness_metadata(&self) -> T0ShmChannelReadiness {
        T0ShmChannelReadiness {
            channel_id: self.id.clone(),
            schema_id: self.schema_id.clone(),
            schema_version: self.schema_version,
            schema_hash: self.schema_hash.clone(),
            slot_size: self.policy.slot_size,
            ownership: self.ownership.as_str().to_string(),
            stale_after_reads: self.policy.stale_after_reads,
            max_spin_retries: self.policy.max_spin_retries,
            max_spin_time_us: self.policy.max_spin_time_us,
            pinned: self.is_pinned(),
            ready: self.ready,
            mapping_path: self.shm.path().display().to_string(),
        }
    }
}

/// Handle-only deterministic T0 transport state.
#[derive(Debug)]
pub struct T0Transport {
    config: T0ShmConfig,
    channels: Vec<T0ChannelState>,
    channel_index: BTreeMap<String, usize>,
    fallback_denied_total: u64,
}

impl Default for T0Transport {
    fn default() -> Self {
        Self::with_config(T0ShmConfig::default())
    }
}

impl T0Transport {
    /// Create an empty T0 transport.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a T0 transport with explicit SHM startup config.
    #[must_use]
    pub fn with_config(config: T0ShmConfig) -> Self {
        Self {
            config,
            channels: Vec::new(),
            channel_index: BTreeMap::new(),
            fallback_denied_total: 0,
        }
    }

    /// Register a fixed-size channel with strict schema hash.
    pub fn register_channel(
        &mut self,
        channel_id: impl Into<String>,
        schema_hash: impl Into<String>,
        policy: T0ChannelPolicy,
    ) -> Result<(), T0Error> {
        self.register_channel_spec(T0ChannelSpec::new(channel_id, schema_hash, policy))
    }

    /// Register an explicit channel contract and allocate/open its OS-backed SHM mapping.
    pub fn register_channel_spec(&mut self, spec: T0ChannelSpec) -> Result<(), T0Error> {
        if spec.channel_id.trim().is_empty() {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "channel_id must not be empty",
            ));
        }
        if spec.schema_id.trim().is_empty() {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "schema_id must not be empty",
            ));
        }
        if spec.schema_hash.trim().is_empty() {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "schema_hash must not be empty",
            ));
        }
        if spec.policy.slot_size == 0 {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "slot_size must be greater than zero",
            ));
        }
        if spec.policy.max_spin_retries == 0 {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "max_spin_retries must be greater than zero",
            ));
        }
        if spec.policy.max_spin_time_us == 0 {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "max_spin_time_us must be greater than zero",
            ));
        }
        if self.channel_index.contains_key(spec.channel_id.as_str()) {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!("channel '{}' already registered", spec.channel_id),
            ));
        }

        let index = self.channels.len();
        let channel_id = spec.channel_id.clone();
        self.channels.push(T0ChannelState::new(spec, &self.config)?);
        self.channel_index.insert(channel_id, index);
        Ok(())
    }

    /// Publish-ready metadata for `_meta/shm_channels`.
    #[must_use]
    pub fn shm_channel_readiness(&self) -> Vec<T0ShmChannelReadiness> {
        self.channels
            .iter()
            .map(T0ChannelState::readiness_metadata)
            .collect()
    }

    /// JSON serialization helper for `_meta/shm_channels` publication payloads.
    pub fn shm_channel_readiness_json(&self) -> Result<String, T0Error> {
        serde_json::to_string(&self.shm_channel_readiness()).map_err(|error| {
            T0Error::new(
                T0ErrorCode::TransportFailure,
                format!("failed to encode SHM channel readiness payload: {error}"),
            )
        })
    }

    /// Bind a publisher handle for a specific channel.
    pub fn bind_publisher(
        &mut self,
        channel_id: &str,
        route: RealtimeRoute,
        schema_hash: &str,
        payload_size: usize,
        fixed_layout: bool,
    ) -> Result<PubHandle, T0Error> {
        let channel_index = self.bind_common(
            channel_id,
            route,
            schema_hash,
            payload_size,
            fixed_layout,
            T0BindMode::Publisher,
        )?;
        let channel = &self.channels[channel_index];
        Ok(PubHandle {
            channel_index,
            payload_size,
            schema_tag: channel.schema_tag,
            route,
        })
    }

    /// Bind a subscriber handle for a specific channel.
    pub fn bind_subscriber(
        &mut self,
        channel_id: &str,
        route: RealtimeRoute,
        schema_hash: &str,
        payload_size: usize,
        fixed_layout: bool,
    ) -> Result<SubHandle, T0Error> {
        let channel_index = self.bind_common(
            channel_id,
            route,
            schema_hash,
            payload_size,
            fixed_layout,
            T0BindMode::Subscriber,
        )?;
        let channel = &self.channels[channel_index];
        Ok(SubHandle {
            channel_index,
            payload_size,
            schema_tag: channel.schema_tag,
            route,
        })
    }

    /// Publish a payload through handle-only HardRT API.
    pub fn publish_hardrt(&mut self, handle: PubHandle, payload: &[u8]) -> Result<(), T0Error> {
        if handle.route != RealtimeRoute::T0HardRt {
            return Err(self.deny_fallback_for_channel(
                handle.channel_index,
                "publish_hardrt requires T0HardRt handle",
            ));
        }
        let channel = self.channel_mut(handle.channel_index)?;
        if !channel.is_ready_for_io() {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!(
                    "channel '{}' is not pinned/ready for T0 publish",
                    channel.id
                ),
            ));
        }
        if !channel.publisher_bound {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!("publisher not bound for channel '{}'", channel.id),
            ));
        }
        if handle.schema_tag != channel.schema_tag {
            return Err(T0Error::new(
                T0ErrorCode::SchemaMismatch,
                format!(
                    "publisher schema mismatch for channel '{}': bound={}, active={}",
                    channel.id, handle.schema_tag, channel.schema_hash
                ),
            ));
        }
        if payload.len() != handle.payload_size {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "payload size {} does not match bound payload size {}",
                    payload.len(),
                    handle.payload_size
                ),
            ));
        }
        if payload.len() > channel.policy.slot_size {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "payload size {} exceeds slot_size {}",
                    payload.len(),
                    channel.policy.slot_size
                ),
            ));
        }

        let write_seq = channel.shm.write_seq();
        let read_seq = channel.shm.read_seq();
        let overrun_count = if write_seq > read_seq {
            channel.counters.overrun_count.saturating_add(1)
        } else {
            channel.counters.overrun_count
        };

        let seq_before = channel.shm.seqlock();
        let seq_odd = if seq_before & 1 == 0 {
            seq_before.saturating_add(1)
        } else {
            seq_before
        };
        channel.shm.set_seqlock(seq_odd);
        channel
            .shm
            .write_payload(payload)
            .map_err(|message| T0Error::new(T0ErrorCode::TransportFailure, message))?;
        channel.shm.set_write_seq(write_seq.saturating_add(1));
        channel.shm.set_overrun_count(overrun_count);
        channel.shm.set_seqlock(seq_odd.saturating_add(1));
        channel.counters.overrun_count = overrun_count;
        Ok(())
    }

    /// Read through handle-only HardRT API with bounded stale/spin semantics.
    pub fn read_hardrt(
        &mut self,
        handle: SubHandle,
        out: &mut [u8],
    ) -> Result<T0ReadOutcome, T0Error> {
        if handle.route != RealtimeRoute::T0HardRt {
            return Err(self.deny_fallback_for_channel(
                handle.channel_index,
                "read_hardrt requires T0HardRt handle",
            ));
        }
        let channel = self.channel_mut(handle.channel_index)?;
        if !channel.is_ready_for_io() {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!("channel '{}' is not pinned/ready for T0 read", channel.id),
            ));
        }
        if !channel.subscriber_bound {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!("subscriber not bound for channel '{}'", channel.id),
            ));
        }
        if handle.schema_tag != channel.schema_tag {
            return Err(T0Error::new(
                T0ErrorCode::SchemaMismatch,
                format!(
                    "subscriber schema mismatch for channel '{}': bound={}, active={}",
                    channel.id, handle.schema_tag, channel.schema_hash
                ),
            ));
        }
        if out.len() < handle.payload_size {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "output buffer size {} is smaller than payload size {}",
                    out.len(),
                    handle.payload_size
                ),
            ));
        }

        let write_seq = channel.shm.write_seq();
        let read_seq = channel.shm.read_seq();
        if write_seq == 0 || write_seq == read_seq {
            channel.stale_misses = channel.stale_misses.saturating_add(1);
            let stale_threshold = channel.policy.stale_after_reads.max(1);
            if channel.stale_misses >= stale_threshold {
                channel.counters.stale_count = channel.counters.stale_count.saturating_add(1);
                return Err(T0Error::new(
                    T0ErrorCode::StaleData,
                    format!(
                        "channel '{}' stale after {} consecutive misses",
                        channel.id, channel.stale_misses
                    ),
                ));
            }
            return Ok(T0ReadOutcome::NoUpdate);
        }

        let read_started = Instant::now();
        let max_spin_time = Duration::from_micros(channel.policy.max_spin_time_us);
        let mut retries = 0_u8;
        loop {
            let seq_before = channel.shm.seqlock();
            if seq_before & 1 == 1 {
                if retries >= channel.policy.max_spin_retries
                    || read_started.elapsed() >= max_spin_time
                {
                    channel.counters.spin_exhausted_count =
                        channel.counters.spin_exhausted_count.saturating_add(1);
                    channel.counters.stale_count = channel.counters.stale_count.saturating_add(1);
                    return Err(T0Error::new(
                        T0ErrorCode::StaleData,
                        format!(
                            "channel '{}' remained writer-unstable after {} retries within {} us",
                            channel.id,
                            channel.policy.max_spin_retries,
                            channel.policy.max_spin_time_us
                        ),
                    ));
                }
                retries = retries.saturating_add(1);
                std::hint::spin_loop();
                continue;
            }

            let bytes = channel
                .shm
                .copy_payload_into(out, handle.payload_size)
                .map_err(|message| T0Error::new(T0ErrorCode::ContractViolation, message))?;
            let seq_after = channel.shm.seqlock();
            if seq_before == seq_after && seq_after & 1 == 0 {
                let newest_write_seq = channel.shm.write_seq();
                let newest_read_seq = channel.shm.read_seq();
                let dropped_updates = newest_write_seq
                    .saturating_sub(newest_read_seq)
                    .saturating_sub(1);
                channel.shm.set_read_seq(newest_write_seq);
                channel.stale_misses = 0;

                return Ok(T0ReadOutcome::Fresh(T0FreshRead {
                    bytes,
                    sequence: newest_write_seq,
                    dropped_updates,
                    overrun_count: channel.shm.overrun_count(),
                }));
            }

            if retries >= channel.policy.max_spin_retries || read_started.elapsed() >= max_spin_time
            {
                channel.counters.spin_exhausted_count =
                    channel.counters.spin_exhausted_count.saturating_add(1);
                channel.counters.stale_count = channel.counters.stale_count.saturating_add(1);
                return Err(T0Error::new(
                    T0ErrorCode::StaleData,
                    format!(
                        "channel '{}' remained unstable after {} retries within {} us",
                        channel.id,
                        channel.policy.max_spin_retries,
                        channel.policy.max_spin_time_us
                    ),
                ));
            }

            retries = retries.saturating_add(1);
            std::hint::spin_loop();
        }
    }

    /// Return counters for a given channel id.
    #[must_use]
    pub fn channel_counters(&self, channel_id: &str) -> Option<T0ChannelCounters> {
        let index = self.channel_index.get(channel_id).copied()?;
        Some(self.channels[index].counters)
    }

    /// Return total denied fallback attempts across all channels.
    #[must_use]
    pub fn fallback_denied_total(&self) -> u64 {
        self.fallback_denied_total
    }

    fn bind_common(
        &mut self,
        channel_id: &str,
        route: RealtimeRoute,
        schema_hash: &str,
        payload_size: usize,
        fixed_layout: bool,
        mode: T0BindMode,
    ) -> Result<usize, T0Error> {
        let channel_index = self.channel_index.get(channel_id).copied().ok_or_else(|| {
            T0Error::new(
                T0ErrorCode::NotConfigured,
                format!("channel '{channel_id}' is not configured"),
            )
        })?;
        if !QosTier::T0HardRt.route_is_legal(route) {
            return Err(self.deny_fallback_for_channel(
                channel_index,
                format!(
                    "T0 bind for channel '{}' forbids mesh/IP fallback route; generic IP mesh is non-HardRT",
                    channel_id
                ),
            ));
        }
        if !fixed_layout {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "channel '{}' requires fixed-layout payload contract for T0",
                    channel_id
                ),
            ));
        }
        if payload_size == 0 {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                "payload_size must be greater than zero",
            ));
        }

        let channel = self.channel_mut(channel_index)?;
        if !channel.ready {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!(
                    "channel '{}' is not startup-ready: SHM allocation/pinning incomplete",
                    channel_id
                ),
            ));
        }
        if !channel.is_pinned() {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!(
                    "channel '{}' is not pinned/ready for HardRT bind",
                    channel_id
                ),
            ));
        }
        if schema_tag(schema_hash) != channel.schema_tag {
            return Err(T0Error::new(
                T0ErrorCode::SchemaMismatch,
                format!(
                    "schema hash mismatch for channel '{}': requested='{}' expected='{}'",
                    channel_id, schema_hash, channel.schema_hash
                ),
            ));
        }
        if payload_size > channel.policy.slot_size {
            return Err(T0Error::new(
                T0ErrorCode::ContractViolation,
                format!(
                    "payload_size {} exceeds configured slot_size {} for channel '{}'",
                    payload_size, channel.policy.slot_size, channel_id
                ),
            ));
        }
        if channel.shm.slot_size() != channel.policy.slot_size {
            return Err(T0Error::new(
                T0ErrorCode::TransportFailure,
                format!(
                    "channel '{}' SHM slot_size contract mismatch: mapped={} configured={}",
                    channel_id,
                    channel.shm.slot_size(),
                    channel.policy.slot_size
                ),
            ));
        }

        match mode {
            T0BindMode::Publisher if channel.publisher_bound => {
                return Err(T0Error::new(
                    T0ErrorCode::ContractViolation,
                    format!("publisher already bound for channel '{}'", channel_id),
                ));
            }
            T0BindMode::Subscriber if channel.subscriber_bound => {
                return Err(T0Error::new(
                    T0ErrorCode::ContractViolation,
                    format!("subscriber already bound for channel '{}'", channel_id),
                ));
            }
            T0BindMode::Publisher if channel.ownership != T0ChannelOwnership::PublisherWrites => {
                return Err(T0Error::new(
                    T0ErrorCode::ContractViolation,
                    format!(
                        "channel '{}' ownership contract forbids publisher-write binding",
                        channel_id
                    ),
                ));
            }
            T0BindMode::Publisher => channel.publisher_bound = true,
            T0BindMode::Subscriber => channel.subscriber_bound = true,
        }

        Ok(channel_index)
    }

    fn deny_fallback_for_channel(
        &mut self,
        channel_index: usize,
        message: impl Into<String>,
    ) -> T0Error {
        if let Some(channel) = self.channels.get_mut(channel_index) {
            channel.counters.fallback_denied_count =
                channel.counters.fallback_denied_count.saturating_add(1);
        }
        self.fallback_denied_total = self.fallback_denied_total.saturating_add(1);
        T0Error::new(T0ErrorCode::ContractViolation, message)
    }

    fn channel_mut(&mut self, index: usize) -> Result<&mut T0ChannelState, T0Error> {
        self.channels.get_mut(index).ok_or_else(|| {
            T0Error::new(
                T0ErrorCode::NotConfigured,
                format!("channel index {} is not configured", index),
            )
        })
    }

    #[cfg(test)]
    pub(super) fn inject_unstable_writer(&mut self, channel_id: &str, _retries: u8) {
        if let Some(index) = self.channel_index.get(channel_id).copied() {
            self.channels[index].shm.set_writer_stuck_for_test();
        }
    }

    #[cfg(test)]
    pub(super) fn inject_unpinned_channel(&mut self, channel_id: &str) {
        if let Some(index) = self.channel_index.get(channel_id).copied() {
            self.channels[index].force_unpinned_for_test = true;
        }
    }
}
