//! Runtime-neutral ADS notification sampling helpers.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::{
    ActiveNotification, AdsErrorCode, NotificationSample, NotificationStamp, NotificationTarget,
    SymbolSource, ValueIo, ADSTRANS_SERVERCYCLE, ADSTRANS_SERVERONCHA,
};

const FILETIME_UNIX_EPOCH_OFFSET_MS: u64 = 11_644_473_600_000;
const FILETIME_TICKS_PER_MILLISECOND: u64 = 10_000;

/// Converts Unix milliseconds to Windows FILETIME ticks.
#[must_use]
pub fn filetime_from_unix_millis(unix_ms: u64) -> u64 {
    unix_ms
        .saturating_add(FILETIME_UNIX_EPOCH_OFFSET_MS)
        .saturating_mul(FILETIME_TICKS_PER_MILLISECOND)
}

/// Samples ADS notifications for one client connection.
#[derive(Debug, Default)]
pub struct NotificationSampler {
    states: BTreeMap<u32, NotificationState>,
}

#[derive(Debug, Clone)]
struct NotificationState {
    last_due_ms: u64,
    last_data: Option<Vec<u8>>,
}

impl NotificationSampler {
    /// Samples registrations that are due at `now_ms`.
    ///
    /// `min_cycle` clamps client-requested zero or very small cycle times.
    ///
    /// # Errors
    ///
    /// Returns `InvalidSize` if notification payload metadata cannot fit the ADS wire.
    pub fn sample(
        &mut self,
        registrations: &[ActiveNotification],
        symbols: &dyn SymbolSource,
        values: &dyn ValueIo,
        now_ms: u64,
        min_cycle: Duration,
    ) -> Result<Option<NotificationStamp>, AdsErrorCode> {
        let min_cycle_ms = millis_at_least_one(min_cycle)?;
        let mut samples = Vec::new();
        let live_handles = registrations
            .iter()
            .map(|registration| registration.handle)
            .collect::<std::collections::BTreeSet<_>>();
        self.states
            .retain(|handle, _state| live_handles.contains(handle));

        for registration in registrations {
            let cycle_ms = effective_cycle_ms(registration.cycle_time_ms, min_cycle_ms);
            let state = self
                .states
                .entry(registration.handle)
                .or_insert(NotificationState {
                    last_due_ms: 0,
                    last_data: None,
                });
            if !is_due(state.last_due_ms, now_ms, cycle_ms) {
                continue;
            }
            state.last_due_ms = now_ms;

            let Ok(data) = sample_registration_data(registration, symbols, values) else {
                samples.push(NotificationSample::invalidated(registration.handle));
                continue;
            };
            let should_emit = match registration.transmission_mode {
                ADSTRANS_SERVERCYCLE => true,
                ADSTRANS_SERVERONCHA => state.last_data.as_deref() != Some(data.as_slice()),
                _ => false,
            };
            if should_emit {
                state.last_data = Some(data.clone());
                samples.push(NotificationSample::new(registration.handle, data));
            }
        }

        if samples.is_empty() {
            Ok(None)
        } else {
            Ok(Some(NotificationStamp::new(
                filetime_from_unix_millis(now_ms),
                samples,
            )))
        }
    }
}

fn sample_registration_data(
    registration: &ActiveNotification,
    symbols: &dyn SymbolSource,
    values: &dyn ValueIo,
) -> Result<Vec<u8>, ()> {
    let byte_len = usize::try_from(registration.byte_len).map_err(|_| ())?;
    match &registration.target {
        NotificationTarget::Symbol(symbol) => match values.read(symbol) {
            Ok((bytes, _quality)) if bytes.len() >= byte_len => Ok(bytes[..byte_len].to_vec()),
            Ok((_bytes, _quality)) => Err(()),
            Err(_error) => Err(()),
        },
        NotificationTarget::SystemBytes(bytes) => {
            if bytes.len() >= byte_len {
                Ok(bytes[..byte_len].to_vec())
            } else {
                Err(())
            }
        }
        NotificationTarget::SymbolVersion => {
            let bytes = symbols.version().to_le_bytes();
            if byte_len <= bytes.len() {
                Ok(bytes[..byte_len].to_vec())
            } else {
                Err(())
            }
        }
    }
}

fn millis_at_least_one(duration: Duration) -> Result<u64, AdsErrorCode> {
    let millis = duration.as_millis().max(1);
    u64::try_from(millis).map_err(|_| AdsErrorCode::InvalidSize)
}

fn effective_cycle_ms(requested_ms: u32, min_cycle_ms: u64) -> u64 {
    u64::from(requested_ms).max(min_cycle_ms)
}

fn is_due(last_due_ms: u64, now_ms: u64, cycle_ms: u64) -> bool {
    last_due_ms == 0 || now_ms.saturating_sub(last_due_ms) >= cycle_ms
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use trust_ads_core::{
        AdsDataTypeDescriptor, IecDataType, PointQuality, SymbolDescriptor, SymbolSnapshot,
    };

    use super::{filetime_from_unix_millis, NotificationSampler};
    use crate::{
        ActiveNotification, AdsErrorCode, AdsServerError, NotificationReceiver, NotificationTarget,
        SymbolSource, ValueIo, ADSTRANS_SERVERCYCLE, ADSTRANS_SERVERONCHA,
    };

    struct SequenceValues {
        values: std::cell::RefCell<Vec<Vec<u8>>>,
    }

    impl ValueIo for SequenceValues {
        fn read(
            &self,
            _symbol: &SymbolDescriptor,
        ) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
            Ok((self.values.borrow_mut().remove(0), PointQuality::good(1)))
        }
    }

    struct StaticSymbols {
        version: u32,
        snapshot: Arc<SymbolSnapshot>,
    }

    impl StaticSymbols {
        fn new(version: u32) -> Self {
            Self {
                version,
                snapshot: Arc::new(SymbolSnapshot::new("test", Vec::new())),
            }
        }
    }

    impl SymbolSource for StaticSymbols {
        fn snapshot(&self) -> Arc<SymbolSnapshot> {
            self.snapshot.clone()
        }

        fn version(&self) -> u32 {
            self.version
        }
    }

    fn registration(handle: u32, mode: u32, cycle_time_ms: u32) -> ActiveNotification {
        ActiveNotification {
            receiver: test_receiver(),
            handle,
            target: NotificationTarget::Symbol(SymbolDescriptor::new(
                "GVL.Setpoint",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                0,
                4,
            )),
            byte_len: 4,
            transmission_mode: mode,
            max_delay_ms: 0,
            cycle_time_ms,
        }
    }

    fn test_receiver() -> NotificationReceiver {
        NotificationReceiver {
            net_id: [1, 2, 3, 4, 5, 6],
            port: 0x8001,
        }
    }

    #[test]
    fn filetime_conversion_matches_unix_epoch_offset() {
        assert_eq!(filetime_from_unix_millis(0), 116_444_736_000_000_000);
        assert_eq!(filetime_from_unix_millis(1), 116_444_736_000_010_000);
    }

    #[test]
    fn server_cycle_emits_every_due_tick() {
        let values = SequenceValues {
            values: std::cell::RefCell::new(vec![
                1.0_f32.to_le_bytes().to_vec(),
                1.0_f32.to_le_bytes().to_vec(),
            ]),
        };
        let mut sampler = NotificationSampler::default();
        let registration = registration(7, ADSTRANS_SERVERCYCLE, 0);
        let symbols = StaticSymbols::new(9);

        assert!(sampler
            .sample(
                std::slice::from_ref(&registration),
                &symbols,
                &values,
                10,
                Duration::from_millis(5)
            )
            .expect("sample")
            .is_some());
        assert!(sampler
            .sample(
                &[registration],
                &symbols,
                &values,
                15,
                Duration::from_millis(5)
            )
            .expect("sample")
            .is_some());
    }

    #[test]
    fn sampler_slices_values_to_registered_watch_length() {
        let values = SequenceValues {
            values: std::cell::RefCell::new(vec![1.0_f32.to_le_bytes().to_vec()]),
        };
        let mut sampler = NotificationSampler::default();
        let mut registration = registration(7, ADSTRANS_SERVERCYCLE, 0);
        registration.byte_len = 2;
        let symbols = StaticSymbols::new(9);

        let stamp = sampler
            .sample(
                &[registration],
                &symbols,
                &values,
                10,
                Duration::from_millis(1),
            )
            .expect("sample")
            .expect("stamp");

        assert_eq!(stamp.samples[0].data, 1.0_f32.to_le_bytes()[..2]);
    }

    #[test]
    fn server_on_change_coalesces_equal_values() {
        let values = SequenceValues {
            values: std::cell::RefCell::new(vec![
                1.0_f32.to_le_bytes().to_vec(),
                1.0_f32.to_le_bytes().to_vec(),
                2.0_f32.to_le_bytes().to_vec(),
            ]),
        };
        let mut sampler = NotificationSampler::default();
        let registration = registration(7, ADSTRANS_SERVERONCHA, 0);
        let symbols = StaticSymbols::new(9);

        assert!(sampler
            .sample(
                std::slice::from_ref(&registration),
                &symbols,
                &values,
                10,
                Duration::from_millis(1)
            )
            .expect("sample")
            .is_some());
        assert!(sampler
            .sample(
                std::slice::from_ref(&registration),
                &symbols,
                &values,
                11,
                Duration::from_millis(1)
            )
            .expect("sample")
            .is_none());
        assert!(sampler
            .sample(
                &[registration],
                &symbols,
                &values,
                12,
                Duration::from_millis(1)
            )
            .expect("sample")
            .is_some());
    }

    #[test]
    fn read_error_invalidates_handle() {
        struct FailingValues;
        impl ValueIo for FailingValues {
            fn read(
                &self,
                _symbol: &SymbolDescriptor,
            ) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
                Err(AdsServerError::device(AdsErrorCode::NotReady, "not ready"))
            }
        }
        let mut sampler = NotificationSampler::default();
        let symbols = StaticSymbols::new(9);
        let stamp = sampler
            .sample(
                &[registration(9, ADSTRANS_SERVERCYCLE, 0)],
                &symbols,
                &FailingValues,
                10,
                Duration::from_millis(1),
            )
            .expect("sample")
            .expect("stamp");

        assert_eq!(stamp.samples[0].handle, 9);
        assert!(stamp.samples[0].data.is_empty());
    }

    #[test]
    fn symbol_version_notification_samples_symbol_source_version() {
        let values = SequenceValues {
            values: std::cell::RefCell::new(Vec::new()),
        };
        let symbols = StaticSymbols::new(258);
        let mut sampler = NotificationSampler::default();
        let registration = ActiveNotification {
            receiver: test_receiver(),
            handle: 17,
            target: NotificationTarget::SymbolVersion,
            byte_len: 1,
            transmission_mode: ADSTRANS_SERVERONCHA,
            max_delay_ms: 0,
            cycle_time_ms: 0,
        };

        let stamp = sampler
            .sample(
                &[registration],
                &symbols,
                &values,
                10,
                Duration::from_millis(1),
            )
            .expect("sample")
            .expect("stamp");

        assert_eq!(stamp.samples[0].handle, 17);
        assert_eq!(stamp.samples[0].data, vec![2]);
    }

    #[test]
    fn system_bytes_notification_samples_without_runtime_read() {
        struct PanickingValues;
        impl ValueIo for PanickingValues {
            fn read(
                &self,
                _symbol: &SymbolDescriptor,
            ) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
                panic!("static system notifications must not read runtime storage");
            }
        }

        let symbols = StaticSymbols::new(9);
        let mut sampler = NotificationSampler::default();
        let registration = ActiveNotification {
            receiver: test_receiver(),
            handle: 18,
            target: NotificationTarget::SystemBytes(1_u32.to_le_bytes().to_vec()),
            byte_len: 4,
            transmission_mode: ADSTRANS_SERVERCYCLE,
            max_delay_ms: 0,
            cycle_time_ms: 0,
        };

        let stamp = sampler
            .sample(
                &[registration],
                &symbols,
                &PanickingValues,
                10,
                Duration::from_millis(1),
            )
            .expect("sample")
            .expect("stamp");

        assert_eq!(stamp.samples[0].handle, 18);
        assert_eq!(stamp.samples[0].data, 1_u32.to_le_bytes());
    }
}
