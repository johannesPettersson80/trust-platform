/// Retain storage backend.
pub trait RetainStore: Send {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError>;
    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError>;
}

pub struct RetainManager {
    store: Option<Box<dyn RetainStore>>,
    save_interval: Option<Duration>,
    last_save: Duration,
    dirty: bool,
    last_snapshot: Option<RetainSnapshot>,
}

impl Default for RetainManager {
    fn default() -> Self {
        Self {
            store: None,
            save_interval: None,
            last_save: Duration::ZERO,
            dirty: false,
            last_snapshot: None,
        }
    }
}

impl std::fmt::Debug for RetainManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainManager")
            .field("store_configured", &self.store.is_some())
            .field("save_interval", &self.save_interval)
            .field("last_save", &self.last_save)
            .field("dirty", &self.dirty)
            .field("has_snapshot", &self.last_snapshot.is_some())
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
        self.save_interval = save_interval;
        self.last_save = now;
        self.dirty = false;
        self.last_snapshot = None;
    }

    pub fn set_save_interval(&mut self, interval: Option<Duration>) {
        self.save_interval = interval;
    }

    /// The configured checkpoint interval, read back for inspection.
    #[must_use]
    pub fn save_interval(&self) -> Option<Duration> {
        self.save_interval
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
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

    pub fn should_save(&self, now: Duration) -> bool {
        let Some(interval) = self.save_interval else {
            return false;
        };
        if !self.dirty {
            return false;
        }
        if interval.as_nanos() <= 0 {
            return true;
        }
        let elapsed = now.as_nanos().saturating_sub(self.last_save.as_nanos());
        elapsed >= interval.as_nanos()
    }

    pub fn save_snapshot(
        &mut self,
        snapshot: RetainSnapshot,
        now: Duration,
    ) -> Result<(), RuntimeError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        if self
            .last_snapshot
            .as_ref()
            .is_some_and(|previous| retained_storage_eq(previous, &snapshot))
        {
            self.dirty = false;
            self.last_save = now;
            return Ok(());
        }
        store.store(&snapshot)?;
        self.last_snapshot = Some(snapshot);
        self.dirty = false;
        self.last_save = now;
        Ok(())
    }
}

/// Logical `Value` equality intentionally ignores opaque physical storage. A
/// retention owner must additionally notice physical-only changes, without
/// allocating a serialized checkpoint just to decide whether to write one.
fn retained_storage_eq(left: &RetainSnapshot, right: &RetainSnapshot) -> bool {
    if left.values().len() != right.values().len() {
        return false;
    }
    left.values().iter().zip(right.values().iter()).all(
        |((left_name, left_value), (right_name, right_value))| {
            left_name == right_name && retained_value_storage_eq(left_value, right_value, 0)
        },
    )
}

fn retained_value_storage_eq(left: &Value, right: &Value, depth: usize) -> bool {
    if depth > trust_runtime_core::bytecode::BYTECODE_MAX_TYPE_NESTING {
        // Do not make an unbounded recursive comparison a save-time hazard.
        return false;
    }
    match (left, right) {
        (Value::Real(left), Value::Real(right)) => left.to_bits() == right.to_bits(),
        (Value::LReal(left), Value::LReal(right)) => left.to_bits() == right.to_bits(),
        (Value::Array(left), Value::Array(right)) => {
            left.dimensions() == right.dimensions()
                && left.elements().len() == right.elements().len()
                && left
                    .elements()
                    .iter()
                    .zip(right.elements())
                    .all(|(left, right)| retained_value_storage_eq(left, right, depth + 1))
        }
        (Value::Struct(left), Value::Struct(right)) => {
            std::sync::Arc::ptr_eq(left, right)
                || (left.physical_checkpoint_eq(right)
                    && left.type_name() == right.type_name()
                    && left.fields().len() == right.fields().len()
                    && left.fields().iter().zip(right.fields().iter()).all(
                        |((left_name, left_value), (right_name, right_value))| {
                            left_name == right_name
                                && retained_value_storage_eq(left_value, right_value, depth + 1)
                        },
                    ))
        }
        _ => left == right,
    }
}
