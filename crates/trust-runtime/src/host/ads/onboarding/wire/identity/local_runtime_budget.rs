use std::time::{Duration, Instant};

const LOCAL_RUNTIME_PROBE_TIMEOUT_MAX: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalRuntimeProbeResult {
    Responded,
    NoResponse,
    DeadlineReached,
}

#[derive(Debug)]
pub(super) struct LocalRuntimeProbeBudget<'a, C: ?Sized> {
    clock: &'a C,
    deadline: Instant,
    probes_started: usize,
}

pub(super) trait MonotonicClock {
    fn now(&self) -> Instant;
}

#[derive(Debug)]
#[cfg(windows)]
pub(super) struct SystemMonotonicClock;

#[cfg(windows)]
impl MonotonicClock for SystemMonotonicClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub(super) trait NativeRuntimeStateProbe {
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), trust_ads_windows::AdsError>;
    fn read_state(
        &mut self,
        target: &trust_ads_windows::AmsAddress,
    ) -> Result<trust_ads_windows::AdsDeviceState, trust_ads_windows::AdsError>;
}

impl NativeRuntimeStateProbe for trust_ads_windows::AdsPort {
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), trust_ads_windows::AdsError> {
        trust_ads_windows::AdsPort::set_timeout(self, timeout)
    }

    fn read_state(
        &mut self,
        target: &trust_ads_windows::AmsAddress,
    ) -> Result<trust_ads_windows::AdsDeviceState, trust_ads_windows::AdsError> {
        trust_ads_windows::AdsPort::read_state(self, target)
    }
}

impl<'a, C: MonotonicClock + ?Sized> LocalRuntimeProbeBudget<'a, C> {
    pub(super) fn new(clock: &'a C, timeout: Duration) -> Self {
        Self::new_at(clock, clock.now(), timeout)
    }

    pub(super) fn new_at(clock: &'a C, now: Instant, timeout: Duration) -> Self {
        Self {
            clock,
            // An unrepresentable caller duration must fail closed instead of
            // turning a bounded discovery request into an unbounded scan.
            deadline: now.checked_add(timeout).unwrap_or(now),
            probes_started: 0,
        }
    }

    pub(super) fn remaining_probe_timeout(&self, probes_remaining: usize) -> Option<Duration> {
        self.remaining_probe_timeout_at(self.clock.now(), probes_remaining)
    }

    pub(super) fn remaining_probe_timeout_at(
        &self,
        now: Instant,
        probes_remaining: usize,
    ) -> Option<Duration> {
        let remaining = self.deadline.checked_duration_since(now)?;
        let remaining_millis = remaining.as_millis();
        if remaining_millis == 0 {
            return None;
        }
        // Reserve one equal share for timeout-application overhead after every
        // still-scheduled identity/port slot. The caller supplies the complete
        // exact/source/collision plan, so a slow early candidate cannot consume
        // the budget reserved for later AMS identities or ports 301/501.
        let shares = u128::try_from(probes_remaining.max(1))
            .unwrap_or(u128::MAX)
            .saturating_add(1);
        // When a machine has many registered runtimes, the complete plan can
        // contain more slots than milliseconds. Still allow bounded 1 ms
        // probes from the strongest candidates; the monotonic deadline stops
        // the scan honestly instead of refusing to start any native call.
        let fair_millis = (remaining_millis / shares).max(1);
        let capped_millis = fair_millis.min(LOCAL_RUNTIME_PROBE_TIMEOUT_MAX.as_millis());
        Some(Duration::from_millis(
            u64::try_from(capped_millis).unwrap_or(u64::MAX),
        ))
    }

    pub(super) fn record_probe_started(&mut self) {
        self.probes_started += 1;
    }

    pub(super) fn completion_is_within_deadline(&self) -> bool {
        self.completion_is_within_deadline_at(self.clock.now())
    }

    pub(super) fn completion_is_within_deadline_at(&self, now: Instant) -> bool {
        now <= self.deadline
    }

    pub(super) fn probes_started(&self) -> usize {
        self.probes_started
    }
}

pub(super) fn budgeted_native_runtime_probe<C: MonotonicClock + ?Sized>(
    budget: &mut LocalRuntimeProbeBudget<'_, C>,
    probe: &mut impl NativeRuntimeStateProbe,
    target: &trust_ads_windows::AmsAddress,
    probes_remaining: usize,
) -> Result<LocalRuntimeProbeResult, String> {
    let Some(mut applied_timeout) = budget.remaining_probe_timeout(probes_remaining) else {
        return Ok(LocalRuntimeProbeResult::DeadlineReached);
    };
    loop {
        probe.set_timeout(applied_timeout).map_err(|error| {
            format!(
                "set native ADS probe timeout for {}:{}: {error}",
                target.net_id, target.port
            )
        })?;
        // AdsSyncSetTimeoutEx is synchronous and cannot itself be pre-empted.
        // Re-sample the monotonic clock and shrink/reapply the timeout whenever
        // that call crossed a whole-millisecond boundary. Consequently the
        // applied timeout is never greater than the measured remaining budget
        // at the point immediately before AdsSyncReadStateReqEx starts.
        let Some(current_timeout) = budget.remaining_probe_timeout(probes_remaining) else {
            return Ok(LocalRuntimeProbeResult::DeadlineReached);
        };
        if current_timeout < applied_timeout {
            applied_timeout = current_timeout;
            continue;
        }
        budget.record_probe_started();
        let result = probe.read_state(target);
        return classify_native_runtime_probe(result, budget.completion_is_within_deadline())
            .map_err(|error| {
                format!(
                    "native ADS read-state probe for {}:{}: {error}",
                    target.net_id, target.port
                )
            });
    }
}

pub(super) fn classify_native_runtime_probe(
    result: Result<trust_ads_windows::AdsDeviceState, trust_ads_windows::AdsError>,
    completed_within_deadline: bool,
) -> Result<LocalRuntimeProbeResult, String> {
    if !completed_within_deadline {
        return Ok(LocalRuntimeProbeResult::DeadlineReached);
    }
    match result {
        Ok(_) => Ok(LocalRuntimeProbeResult::Responded),
        // A device-level ADS error is still a reply from the candidate service
        // and therefore proves that the AMS identity exists.
        Err(trust_ads_windows::AdsError::Call {
            code: 0x700..=0x73F,
            ..
        }) => Ok(LocalRuntimeProbeResult::Responded),
        // These router/client outcomes mean that this candidate or logical port
        // did not answer. They are expected while scanning collision IDs/ports.
        Err(trust_ads_windows::AdsError::Call {
            code: 0x006 | 0x007 | 0x015 | 0x01B | 0x507 | 0x745,
            ..
        }) => Ok(LocalRuntimeProbeResult::NoResponse),
        // Router state, client-port, ABI, and all other native failures are not
        // evidence that a device is absent; surface them instead of hiding them.
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use std::cell::Cell;
    use std::collections::{BTreeMap, VecDeque};

    use super::super::{responding_runtime_targets, windows_runtime, LocalRuntimeIdentityReport};
    use super::*;

    #[derive(Debug)]
    pub(crate) struct FakeClock {
        origin: Instant,
        elapsed: Cell<Duration>,
    }

    impl FakeClock {
        pub(crate) fn new() -> Self {
            Self {
                origin: Instant::now(),
                elapsed: Cell::new(Duration::ZERO),
            }
        }

        fn advance(&self, duration: Duration) {
            self.elapsed
                .set(self.elapsed.get().saturating_add(duration));
        }
    }

    impl MonotonicClock for FakeClock {
        fn now(&self) -> Instant {
            self.origin + self.elapsed.get()
        }
    }

    #[derive(Clone)]
    pub(crate) enum ScriptedDelay {
        Fixed(Duration),
        AppliedTimeout,
    }

    #[derive(Clone)]
    pub(crate) struct ScriptedReply {
        delay: ScriptedDelay,
        result: Result<trust_ads_windows::AdsDeviceState, trust_ads_windows::AdsError>,
    }

    #[derive(Debug)]
    pub(crate) struct RecordedProbe {
        pub(crate) net_id: String,
        pub(crate) port: u16,
        pub(crate) timeout: Duration,
        pub(crate) started_after: Duration,
    }

    pub(crate) struct ScriptedNativeProbe<'a> {
        clock: &'a FakeClock,
        set_timeout_delay: Duration,
        applied_timeout: Option<Duration>,
        replies: BTreeMap<(String, u16), VecDeque<ScriptedReply>>,
        pub(crate) calls: Vec<RecordedProbe>,
    }

    impl<'a> ScriptedNativeProbe<'a> {
        pub(crate) fn new(clock: &'a FakeClock) -> Self {
            Self {
                clock,
                set_timeout_delay: Duration::ZERO,
                applied_timeout: None,
                replies: BTreeMap::new(),
                calls: Vec::new(),
            }
        }

        pub(crate) fn reply(mut self, net_id: &str, port: u16, reply: ScriptedReply) -> Self {
            self.replies
                .entry((net_id.to_string(), port))
                .or_default()
                .push_back(reply);
            self
        }

        pub(crate) fn with_set_timeout_delay(mut self, delay: Duration) -> Self {
            self.set_timeout_delay = delay;
            self
        }
    }

    impl NativeRuntimeStateProbe for ScriptedNativeProbe<'_> {
        fn set_timeout(&mut self, timeout: Duration) -> Result<(), trust_ads_windows::AdsError> {
            self.clock.advance(self.set_timeout_delay);
            self.applied_timeout = Some(timeout);
            Ok(())
        }

        fn read_state(
            &mut self,
            target: &trust_ads_windows::AmsAddress,
        ) -> Result<trust_ads_windows::AdsDeviceState, trust_ads_windows::AdsError> {
            let timeout = self
                .applied_timeout
                .expect("timeout applied before read-state");
            let net_id = target.net_id.to_string();
            self.calls.push(RecordedProbe {
                net_id: net_id.clone(),
                port: target.port,
                timeout,
                started_after: self.clock.elapsed.get(),
            });
            let reply = self
                .replies
                .get_mut(&(net_id, target.port))
                .and_then(VecDeque::pop_front)
                .unwrap_or_else(no_port_reply);
            self.clock.advance(match reply.delay {
                ScriptedDelay::Fixed(delay) => delay,
                ScriptedDelay::AppliedTimeout => timeout,
            });
            reply.result
        }
    }

    fn native_reply(
        delay: ScriptedDelay,
        result: Result<trust_ads_windows::AdsDeviceState, trust_ads_windows::AdsError>,
    ) -> ScriptedReply {
        ScriptedReply { delay, result }
    }

    pub(crate) fn running_reply(delay: ScriptedDelay) -> ScriptedReply {
        native_reply(
            delay,
            Ok(trust_ads_windows::AdsDeviceState {
                ads_state: 5,
                device_state: 0,
            }),
        )
    }

    pub(crate) fn error_reply(
        code: i32,
        description: &'static str,
        delay: ScriptedDelay,
    ) -> ScriptedReply {
        native_reply(
            delay,
            Err(trust_ads_windows::AdsError::Call {
                operation: "AdsSyncReadStateReqEx",
                code,
                description,
            }),
        )
    }

    fn no_port_reply() -> ScriptedReply {
        error_reply(
            0x006,
            "target ADS port was not found",
            ScriptedDelay::Fixed(Duration::ZERO),
        )
    }

    pub(crate) fn configured_runtime() -> windows_runtime::ConfiguredRuntime {
        windows_runtime::ConfiguredRuntime {
            name: "UmRT_Default".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
        }
    }

    pub(crate) fn scan_with_script(
        clock: &FakeClock,
        probe: &mut ScriptedNativeProbe<'_>,
        timeout: Duration,
        configured: &[windows_runtime::ConfiguredRuntime],
        source_net_id: &str,
    ) -> Result<LocalRuntimeIdentityReport, String> {
        let mut budget = LocalRuntimeProbeBudget::new(clock, timeout);
        responding_runtime_targets(
            "127.0.0.1",
            configured,
            source_net_id,
            |net_id, port, ports_remaining| {
                let net_id = net_id
                    .parse::<trust_ads_windows::AmsNetId>()
                    .map_err(|error| error.to_string())?;
                budgeted_native_runtime_probe(
                    &mut budget,
                    probe,
                    &trust_ads_windows::AmsAddress::new(net_id, port),
                    ports_remaining,
                )
            },
        )
    }
}
