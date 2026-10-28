//! The runtime's HMI bundles (specification 39, §16.2.3–5, §16.4): the active bundle is the
//! project's `hmi/` folder and the previous one is kept beside it. Activation and rollback switch
//! them in one step under a lock that every HMI read takes, so no read sees a folder half
//! switched, and a failure leaves the active bundle as it was.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use super::HmiBundle;

/// The previous bundle, beside `hmi/`.
const PREVIOUS: &str = ".hmi-previous";

/// The active and previous versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions {
    /// The active bundle's version, if there is an `hmi/` folder.
    pub active: Option<String>,
    /// The previous bundle's version, if one is kept.
    pub previous: Option<String>,
}

/// The bundles of one project folder.
#[derive(Debug)]
pub struct BundleStore {
    root: PathBuf,
    lock: RwLock<()>,
}

impl BundleStore {
    /// The store of a project folder.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: RwLock::new(()),
        }
    }

    /// Reads the project's HMI files under the lock.
    pub fn read<T>(&self, read: impl FnOnce(&Path) -> T) -> T {
        let _guard = self
            .lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        read(&self.root)
    }

    /// Makes a bundle the active one and keeps the active one as the previous. The bundle is
    /// written in full before anything switches; if the switch fails, the active bundle is put
    /// back. The version already active changes nothing, so the previous bundle stays.
    pub fn activate(&self, bundle: &HmiBundle) -> std::io::Result<Versions> {
        self.activate_with_invalidation(bundle, || {})
    }

    pub(crate) fn activate_with_invalidation(
        &self,
        bundle: &HmiBundle,
        invalidate: impl FnOnce(),
    ) -> std::io::Result<Versions> {
        {
            let _guard = self
                .lock
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let versions = self.versions_unlocked();
            if versions.active.as_deref() == Some(bundle.version().as_str()) {
                return Ok(versions);
            }
        }
        static STAGED: AtomicU64 = AtomicU64::new(0);
        let staging = self.root.join(format!(
            ".hmi-staging-{}-{}",
            std::process::id(),
            STAGED.fetch_add(1, Ordering::Relaxed)
        ));
        if staging.exists() {
            std::fs::remove_dir_all(&staging)?;
        }
        if let Err(error) = bundle.write(&staging) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        let _guard = self
            .lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (active, previous) = (self.active(), self.previous());
        invalidate();
        let switched = (|| {
            if previous.exists() {
                std::fs::remove_dir_all(previous.parent().unwrap_or(&previous))?;
            }
            if active.exists() {
                std::fs::create_dir_all(previous.parent().unwrap_or(&self.root))?;
                std::fs::rename(&active, &previous)?;
            }
            std::fs::rename(staging.join("hmi"), &active)
        })();
        if let Err(error) = switched {
            if !active.exists() && previous.exists() {
                let _ = std::fs::rename(&previous, &active);
            }
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        let _ = std::fs::remove_dir_all(&staging);
        Ok(self.versions_unlocked())
    }

    /// Makes the previous bundle active again, keeping the active one as the previous.
    pub fn rollback(&self) -> Result<Versions, String> {
        self.rollback_with_invalidation(|| {})
    }

    pub(crate) fn rollback_with_invalidation(
        &self,
        invalidate: impl FnOnce(),
    ) -> Result<Versions, String> {
        let _guard = self
            .lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (active, previous) = (self.active(), self.previous());
        if !previous.exists() {
            return Err("there is no previous bundle to go back to".to_string());
        }
        invalidate();
        let swap = self.root.join(".hmi-swap").join("hmi");
        let moved = (|| {
            if let Some(parent) = swap.parent() {
                if parent.exists() {
                    std::fs::remove_dir_all(parent)?;
                }
                std::fs::create_dir_all(parent)?;
            }
            if active.exists() {
                std::fs::rename(&active, &swap)?;
            }
            std::fs::rename(&previous, &active)?;
            if swap.exists() {
                std::fs::rename(&swap, &previous)?;
            }
            std::fs::remove_dir_all(self.root.join(".hmi-swap"))
        })();
        moved.map_err(|error| format!("the rollback failed: {error}"))?;
        Ok(self.versions_unlocked())
    }

    /// The active and previous versions.
    pub fn versions(&self) -> Versions {
        let _guard = self
            .lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.versions_unlocked()
    }

    fn versions_unlocked(&self) -> Versions {
        let version = |project: &Path| {
            HmiBundle::read(project)
                .ok()
                .filter(|bundle| !bundle.files.is_empty())
                .map(|bundle| bundle.version())
        };
        Versions {
            active: version(&self.root),
            previous: version(&self.root.join(PREVIOUS)),
        }
    }

    fn active(&self) -> PathBuf {
        self.root.join("hmi")
    }

    /// The previous bundle's folder, kept as `.hmi-previous/hmi` so it reads like a project's.
    fn previous(&self) -> PathBuf {
        self.root.join(PREVIOUS).join("hmi")
    }
}
