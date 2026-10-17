use trust_runtime_core::retain::capture::RetainCapture;

/// Retain storage backend.
pub trait RetainStore: Send {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError>;
    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError>;
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
