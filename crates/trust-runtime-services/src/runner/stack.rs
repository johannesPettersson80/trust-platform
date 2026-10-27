//! A device's main stack (specification 34 MEM-03, Appendix D.2; specification 44 §8.5): the
//! window the device paints and measures, and what its own path takes of it, from which the task
//! stack that a compiled program's stack bound must fit follows.

/// A device's main stack: the window the device paints and measures, and the bytes the deepest
/// path of the firmware takes before a program's entry point runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStack {
    /// Bytes of the main-stack window that the device paints and measures.
    pub window_bytes: usize,
    /// Bytes the deepest path of the firmware takes before a program's entry point runs, measured on
    /// the bench.
    pub base_bytes: usize,
}

impl DeviceStack {
    /// The headroom the deepest path keeps below the window's bottom (Appendix D.2).
    pub const HEADROOM_BYTES: usize = 2 * 1024;

    /// The bytes of the window a program's entry points may take: the window less the headroom and
    /// the firmware's own path (specification 44 §8.5).
    #[must_use]
    pub const fn task_stack_bytes(self) -> usize {
        self.window_bytes
            .saturating_sub(Self::HEADROOM_BYTES)
            .saturating_sub(self.base_bytes)
    }
}
