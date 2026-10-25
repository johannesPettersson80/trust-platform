//! What the C6's loader of compiled programs ([`crate::compiled_loader`]) needs of the chip and takes
//! from a program's record (specification 34 STATE-08 and §11.2; specification 44 §10): the hardware
//! that claims a record's lines and the checkpoint flash, the hardware watchdog set from the record's
//! cycle watchdog, the memory the C6 keeps beside a program, and the settings, the checkpoint store and
//! the event store's capacity that a record configures.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::CheckpointStore;
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, CheckpointIdentity};
use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::runner::{CheckpointBuffer, CycleSettings, ProcessIo, RetainConfig};

use crate::profile::{CHECKPOINT_STEP_BYTES, DEVICE_DESCRIPTOR};

/// What the loader needs of the chip's hardware: the lines of a record and the checkpoint flash.
pub trait ProgramHardware {
    /// The process I/O that the lines of a record make. Dropping it frees the pins of its lines, so the
    /// program loaded next claims them (specification 34 STATE-08).
    type Io: ProcessIo;

    /// Claims the lines of `config` by pin name, in `io.toml` order, drives each output to its initial
    /// value before it becomes an output (specification 34 STATE-01 and TARGET-01) and binds them to the
    /// process image.
    ///
    /// # Errors
    ///
    /// The reason a line cannot be claimed. The pins of the lines before it are free again.
    fn claim_lines(&mut self, config: &Configuration) -> Result<Self::Io, String>;

    /// The checkpoint flash for a program whose record keeps retained values: one writer at a time, free
    /// again when the program that held it is dropped.
    ///
    /// # Errors
    ///
    /// The reason the checkpoint flash is not available.
    fn checkpoint_flash(&mut self) -> Result<Box<dyn CheckpointFlash>, String>;
}

/// The bytes of the heap that an online change keeps free for its working memory, beyond the running
/// program and the new one (specification 34 §11.3, UPDATE-06 and Appendix D.2; specification 44
/// §12.3): the change's areas must fit in the heap's room now less these bytes. What it takes at its
/// peak beyond the two programs stays inside the heap.
///
/// The size was measured with a counting allocator on the bytecode engine's online changes of five
/// programs on the host, which held at most 3,994 bytes at once beyond the running program and the
/// candidate (20 October 2026); twice that, for the fragmentation of a first-fit heap, rounded up to
/// 8 KiB. A compiled program's change keeps its candidate's areas in the heap and its image where its
/// upload lies, which the C6 bench changed online three times in a row within it (specification 44
/// §12.3, 24 October 2026).
pub const ONLINE_CHANGE_WORKING_MEMORY_BYTES: usize = 8 * 1024;

/// The memory the C6 has for a program (specification 34 MEM-12, Appendix D.2), which its descriptor
/// reports and its admission compares a program's memory figure with: the heap's `free` bytes once
/// the resource loop is built, with the program loader and the retained-value check the firmware
/// gives it, and before any container is read, less what the chip keeps beside any program besides
/// its loop. That is the working memory of an online change
/// ([`ONLINE_CHANGE_WORKING_MEMORY_BYTES`]), which also holds the program's Live Values and services,
/// and `kept_aside`, the checkpoint store's payload buffer when the chip has not taken it at boot
/// (the largest payload a checkpoint slot takes, which a warm start also reads the newest
/// checkpoint into), or 0 when it has, as its bytes are no longer free.
#[must_use]
pub fn program_memory_bytes(free: usize, kept_aside: usize) -> usize {
    free.saturating_sub(ONLINE_CHANGE_WORKING_MEMORY_BYTES.saturating_add(kept_aside))
}

/// The checkpoint store of a program whose record keeps retained values, over the checkpoint flash of
/// `hardware`, with the identity that its checkpoints carry (specification 34 §10; specification 44
/// §11).
pub(crate) fn retain_config<H: ProgramHardware>(
    hardware: &mut H,
    config: &Configuration,
    identity: CheckpointIdentity,
    buffer: Option<CheckpointBuffer>,
) -> Result<RetainConfig, String> {
    let flash = hardware.checkpoint_flash()?;
    // The largest payload one slot takes: a retained image is saved whole or not at all.
    let image_bytes = usize::try_from(flash.geometry().max_payload_len()).map_err(|_| {
        String::from("the checkpoint's largest payload exceeds the platform's address range")
    })?;
    Ok(RetainConfig {
        mode: RetainMode::File,
        store: CheckpointStore {
            flash,
            program_budget: CHECKPOINT_STEP_BYTES,
        },
        identity,
        save_interval: config.retain_save_interval,
        image_bytes,
        buffer,
    })
}

/// The refusal of a program that cannot start: its checkpoint log cannot be opened when the record
/// keeps retained values, and otherwise the program itself cannot be constructed.
pub(crate) fn start_failure(retains: bool, error: &RuntimeError) -> String {
    if retains {
        format!("the installed application's checkpoint cannot be opened: {error}")
    } else {
        format!("the installed application cannot be started: {error}")
    }
}

/// What the deployment configures for the program besides its code (specification 34 §5.1 and
/// TARGET-08): the cycle interval, the watchdog and fault policies, and the safe value of each output.
pub(crate) fn settings(config: &Configuration) -> CycleSettings {
    CycleSettings {
        hang_action: config.watchdog_hang_action,
        cycle_interval: Duration::from_nanos(
            i64::try_from(config.cycle_interval_nanos).unwrap_or(i64::MAX),
        ),
        watchdog: config.watchdog,
        fault_policy: config.fault_policy,
        safe_outputs: config
            .safe_state
            .iter()
            .map(|entry| (entry.address.clone(), entry.value.clone()))
            .collect(),
    }
}

/// The newest events the event store keeps: the record's section-7 capacity, or the C6's default when
/// the record sets none (0). Admission has already refused a capacity above the chip's maximum
/// (specification 34 §5.1.1 step 7; specification 11, the event store).
#[must_use]
pub fn event_capacity(config: &Configuration) -> usize {
    if config.event_capacity == 0 {
        default_event_capacity()
    } else {
        usize::try_from(config.event_capacity).unwrap_or(usize::MAX)
    }
}

/// The C6's default event-store capacity, from its descriptor (`chips/esp32c6.toml`).
#[must_use]
pub fn default_event_capacity() -> usize {
    usize::try_from(DEVICE_DESCRIPTOR.event_store_default).unwrap_or(usize::MAX)
}
