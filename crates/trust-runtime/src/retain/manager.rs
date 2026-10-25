use trust_runtime_core::retain::capture::RetainCapture;

/// Retain storage backend.
pub trait RetainStore: Send {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError>;
    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError>;

    /// Stores the STRN v3 `payload` a capture staged, as [`RetainStore::store`] stores the snapshot
    /// it holds: a compiled program's capture writes the payload itself (specification 44 §11).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when the payload does not decode or cannot be stored.
    fn store_payload(&self, payload: &[u8]) -> Result<(), RuntimeError> {
        let snapshot = trust_runtime_core::retain::codec::decode_snapshot_payload_v3(payload)?;
        self.store(&RetainSnapshot::from_values(snapshot.into_values()))
    }
}

/// The hosted persistence adapter: a store plus the capture rule shared with the chips
/// (`trust-runtime-core::retain::capture`, specification 11 §6.7, specification 34 §10).
pub struct RetainManager {
    store: Option<Box<dyn RetainStore>>,
    capture: RetainCapture,
}

impl Default for RetainManager {
    fn default() -> Self {
        Self {
            store: None,
            capture: RetainCapture::new(),
        }
    }
}

impl std::fmt::Debug for RetainManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainManager")
            .field("store_configured", &self.store.is_some())
            .field("save_interval", &self.capture.interval())
            .field("last_save", &self.capture.last_save())
            .field("dirty", &self.capture.is_dirty())
            .field("has_snapshot", &self.capture.has_image())
            .finish()
    }
}

impl RetainManager {
    pub fn configure(
        &mut self,
        store: Option<Box<dyn RetainStore>>,
        save_interval: Option<Duration>,
        now: Duration,
    ) {
        self.store = store;
        self.capture.configure(save_interval, now);
    }

    pub fn set_save_interval(&mut self, interval: Option<Duration>) {
        self.capture.set_interval(interval);
    }

    /// The configured checkpoint interval, read back for inspection.
    #[must_use]
    pub fn save_interval(&self) -> Option<Duration> {
        self.capture.interval()
    }

    pub fn mark_dirty(&mut self) {
        self.capture.mark_dirty();
    }

    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    pub fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(RetainSnapshot::default());
        };
        store.load()
    }

    /// Whether the shared capture rule wants a checkpoint now (specification 11 §6.7).
    pub fn should_save(&self, now: Duration) -> bool {
        self.capture.should_save(now)
    }

    /// Stage `snapshot` in the shared capture and let its unchanged-image skip decide whether the
    /// store is written (specification 11 §6.7). A failed write leaves the snapshot dirty, so a
    /// later save may retry.
    pub fn save_snapshot(
        &mut self,
        snapshot: RetainSnapshot,
        now: Duration,
    ) -> Result<(), RuntimeError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        if !stage_image(&mut self.capture, snapshot.values())? {
            self.capture.unchanged(now);
            return Ok(());
        }
        store.store(&snapshot)?;
        self.capture.accept(now);
        Ok(())
    }
}

/// Stage the engine's captured image in the shared capture and report whether it changed
/// (specification 11 §6.7).
///
/// The hosted adapter captures the image through the engine's own snapshot, so this is where the
/// host's named values meet the portable capture.
fn stage_image(
    capture: &mut RetainCapture,
    values: &indexmap::IndexMap<smol_str::SmolStr, Value>,
) -> Result<bool, RuntimeError> {
    capture.stage_values(values)?;
    Ok(capture.image_changed())
}

/// A compiled program's retained values go to the same store, with the same capture rule, as the
/// bytecode engine's (specification 44 §11, §8.8): a scan marks them dirty and a due capture writes
/// them when they changed; a stop writes them when they changed; a warm restart loads them.
impl trust_runtime_services::runner::CycleRetain for RetainManager {
    fn persists(&self) -> bool {
        self.store.is_some()
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        if self.store.is_none() {
            return Ok(());
        }
        self.capture.reserve(bytes)
    }

    fn capture_with(
        &mut self,
        now: Duration,
        fill: &mut dyn FnMut(&mut RetainCapture) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if self.store.is_none() {
            return Ok(());
        }
        self.capture.mark_dirty();
        if !self.capture.should_save(now) {
            return Ok(());
        }
        self.write_captured(now, fill)
    }

    /// A computer writes its retain file at once, so no save waits for a step.
    fn step(&mut self) {}

    fn set_save_interval(&mut self, interval: Option<Duration>) {
        RetainManager::set_save_interval(self, interval);
    }

    fn save_with(
        &mut self,
        now: Duration,
        fill: &mut dyn FnMut(&mut RetainCapture) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if self.store.is_none() {
            return Ok(());
        }
        self.write_captured(now, fill)
    }

    fn recover(&mut self) -> Result<trust_runtime_core::retain::RetainSnapshot, RuntimeError> {
        self.load().map(|snapshot| snapshot.inner)
    }
}

impl RetainManager {
    /// Captures the values with `fill` and writes them when they differ from the values written
    /// last (specification 11 §6.7). A failed write leaves the capture dirty, so a later save
    /// retries.
    fn write_captured(
        &mut self,
        now: Duration,
        fill: &mut dyn FnMut(&mut RetainCapture) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        fill(&mut self.capture)?;
        if !self.capture.image_changed() {
            self.capture.unchanged(now);
            return Ok(());
        }
        store.store_payload(self.capture.staged_payload())?;
        self.capture.accept(now);
        Ok(())
    }
}
