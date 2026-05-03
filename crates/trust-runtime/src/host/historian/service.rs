impl HistorianService {
    pub fn new(
        config: HistorianConfig,
        bundle_root: Option<&Path>,
    ) -> Result<Arc<Self>, RuntimeError> {
        let history_path = resolve_path(&config.history_path, bundle_root);
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                RuntimeError::ControlError(format!("historian path setup failed: {err}").into())
            })?;
        }
        let include_patterns = compile_patterns(&config.include)?;
        let alert_rules = compile_alert_rules(&config.alerts, bundle_root)?;

        let mut inner = HistorianInner::default();
        load_existing_samples(&history_path, config.max_entries, &mut inner)?;

        let mut runtime_config = config.clone();
        runtime_config.history_path = history_path;

        Ok(Arc::new(Self {
            config: runtime_config,
            include_patterns,
            alert_rules,
            inner: Mutex::new(inner),
        }))
    }

    #[must_use]
    pub fn config(&self) -> &HistorianConfig {
        &self.config
    }

    pub fn start_sampler(self: Arc<Self>, debug: crate::debug::DebugControl) {
        let interval = self.config.sample_interval_ms.max(1);
        let poll_ms = (interval / 2).clamp(10, 1_000);
        thread::spawn(move || loop {
            if let Some(snapshot) = debug.snapshot() {
                let now_ms = unix_ms();
                let _ = self.capture_snapshot_at(&snapshot, now_ms);
            }
            thread::sleep(Duration::from_millis(poll_ms));
        });
    }

    pub fn capture_snapshot_at(
        &self,
        snapshot: &DebugSnapshot,
        timestamp_ms: u128,
    ) -> Result<usize, RuntimeError> {
        let interval_ms = u128::from(self.config.sample_interval_ms.max(1));
        let mut pending_hooks: Vec<(HookTarget, HistorianAlertEvent)> = Vec::new();

        let recorded = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| RuntimeError::ControlError("historian unavailable".into()))?;
            if let Some(last) = inner.last_capture_ms {
                if timestamp_ms.saturating_sub(last) < interval_ms {
                    return Ok(0);
                }
            }

            let samples = collect_snapshot_samples(
                snapshot,
                &self.config,
                &self.include_patterns,
                timestamp_ms,
            );
            if samples.is_empty() {
                inner.last_capture_ms = Some(timestamp_ms);
                return Ok(0);
            }

            append_samples(&self.config.history_path, &samples)?;
            for sample in &samples {
                inner.samples.push_back(sample.clone());
                inner.tracked_variables.insert(sample.variable.clone());
                while inner.samples.len() > self.config.max_entries {
                    let _ = inner.samples.pop_front();
                }
            }
            inner.samples_total = inner.samples_total.saturating_add(samples.len() as u64);
            inner.last_capture_ms = Some(timestamp_ms);

            let mut latest_numeric = HashMap::<String, f64>::new();
            for sample in &samples {
                if let Some(value) = sample.value.as_f64() {
                    latest_numeric.insert(sample.variable.clone(), value);
                }
            }
            let alert_events = evaluate_alerts(
                &self.alert_rules,
                &latest_numeric,
                timestamp_ms,
                &mut inner.alert_trackers,
            );
            for (event, hook) in alert_events {
                inner.alerts.push_back(event.clone());
                while inner.alerts.len() > 1_000 {
                    let _ = inner.alerts.pop_front();
                }
                inner.alerts_total = inner.alerts_total.saturating_add(1);
                if let Some(target) = hook {
                    pending_hooks.push((target, event));
                }
            }

            samples.len()
        };

        for (target, event) in pending_hooks {
            dispatch_hook(&target, &event);
        }

        Ok(recorded)
    }

    #[must_use]
    pub fn query(
        &self,
        variable: Option<&str>,
        since_ms: Option<u128>,
        limit: usize,
    ) -> Vec<HistorianSample> {
        let limit = limit.clamp(1, 5_000);
        let Ok(inner) = self.inner.lock() else {
            return Vec::new();
        };
        let mut items = inner
            .samples
            .iter()
            .rev()
            .filter(|sample| variable.is_none_or(|name| sample.variable.as_str() == name))
            .filter(|sample| since_ms.is_none_or(|value| sample.timestamp_ms >= value))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        items.reverse();
        items
    }

    #[must_use]
    pub fn alerts(&self, limit: usize) -> Vec<HistorianAlertEvent> {
        let limit = limit.clamp(1, 1_000);
        let Ok(inner) = self.inner.lock() else {
            return Vec::new();
        };
        let mut items = inner
            .alerts
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        items.reverse();
        items
    }

    #[must_use]
    pub fn prometheus_path(&self) -> Option<&str> {
        if self.config.prometheus_enabled {
            Some(self.config.prometheus_path.as_str())
        } else {
            None
        }
    }

    #[must_use]
    pub fn prometheus_snapshot(&self) -> HistorianPrometheusSnapshot {
        let Ok(inner) = self.inner.lock() else {
            return HistorianPrometheusSnapshot::default();
        };
        HistorianPrometheusSnapshot {
            samples_total: inner.samples_total,
            series_total: inner.tracked_variables.len(),
            alerts_total: inner.alerts_total,
        }
    }

    #[must_use]
    pub fn render_prometheus(&self, runtime: &RuntimeMetricsSnapshot) -> String {
        render_prometheus(runtime, Some(self.prometheus_snapshot()))
    }
}
