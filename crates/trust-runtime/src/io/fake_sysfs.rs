//! A temporary folder that stands in for `/sys` in the tests of the Linux drivers `analog`, `pwm`,
//! `counter` and `can`, which take the sysfs root as a parameter: a test writes the attributes a
//! kernel would show and reads back what the driver wrote. Such a test proves the drivers' use of
//! the files only, not a kernel driver, a voltage, the wiring or the timing (specification 11
//! §6.5); those need the real-hardware case.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Tells the folders of tests that run at the same time apart.
static SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A temporary sysfs tree, removed when dropped.
pub(super) struct FakeSysfs {
    root: PathBuf,
}

impl FakeSysfs {
    /// An empty tree named after `name`.
    pub(super) fn new(name: &str) -> Self {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-sysfs-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create the fake sysfs tree");
        Self { root }
    }

    /// The tree's root, the driver's sysfs root.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    /// Writes `text` to the attribute at `relative`, with the folders above it.
    pub(super) fn set(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        let folder = path.parent().expect("an attribute has a folder");
        fs::create_dir_all(folder).expect("create the attribute's folder");
        fs::write(path, text).expect("write the attribute");
    }

    /// The attribute at `relative`, as the driver left it.
    pub(super) fn get(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("read the attribute")
    }

    /// Removes the attribute at `relative`, as when a device goes away.
    pub(super) fn remove(&self, relative: &str) {
        fs::remove_file(self.root.join(relative)).expect("remove the attribute");
    }
}

impl Drop for FakeSysfs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
