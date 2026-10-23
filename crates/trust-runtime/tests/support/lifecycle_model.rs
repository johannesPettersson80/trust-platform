//! Explicit state-transition model for the stateful lifecycle sequences (scope H, checklist A08).
//!
//! The model is written from the specifications, not from the product code. The rules cite spec 10
//! section 4.7 (the scan transaction), section 4.8 (retain, restart and online change), spec 11
//! sections 6.6 (fault), 6.7 and 6.7.1 (retain storage and online change), 6.9 and 6.9.2 (debug
//! mutation lifecycle), and spec 34 HOST-09, UPDATE-01, UPDATE-02 and UPDATE-07.
//!
//! Operations the specifications leave open are not in [`Op`]. [`Model::allows`] says which
//! operations the specifications define in a given state, and the sequence driver skips the others.
//!
//! The fixture programs are in `stateful_lifecycle_sequences.rs`. Program A is installed first.
//! Program B is a compatible replacement. Program C is an incompatible replacement: `Main.r` changes
//! from INT to BOOL. Retained: `g_r` (global) and `Main.r`. Non-retained: `i`, `d`, `n`, `q` and the
//! `ident` global. `out` is the process output `%QX0.0`.

const R_INIT: i16 = 1;
const G_R_INIT: i16 = 5;
const N_INIT: i16 = 2;
const Q_INIT: i16 = 0;
const I_INIT: i16 = 0;
const D_INIT: i16 = 1;

/// The installed application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exe {
    A,
    B,
}

impl Exe {
    /// The initial value of the program's `ident` global, which differs between the programs.
    #[must_use]
    pub fn ident(self) -> i16 {
        match self {
            Self::A => 1,
            Self::B => 2,
        }
    }
}

/// One generated operation. The payloads are the values written, forced or queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// One scan cycle.
    Scan,
    /// Checked write of the non-retained global `i`.
    WriteI(i16),
    /// Checked write of the subrange global `d` (0..=10). Other values are refused.
    WriteD(i16),
    /// Queue a debugger write of `i`, applied at the next scan boundary.
    EnqueueI(i16),
    /// Queue a debugger write of `d`, applied at the next scan boundary.
    EnqueueD(i16),
    /// Force `i` at every scan boundary until released.
    ForceI(i16),
    /// Force `d` at every scan boundary until released.
    ForceD(i16),
    ReleaseI,
    ReleaseD,
    /// Debugger hold: no scan runs until `Resume`.
    Pause,
    /// Release the debugger hold.
    Resume,
    /// Warm restart without reading the retain store.
    RestartWarm,
    /// Cold restart.
    RestartCold,
    /// Warm restart that loads the retain store.
    RestartWarmFromStore,
    /// Save the retained values to the store.
    SaveStore,
    /// Replace the installed program with B through the online change transaction.
    OnlineChangeB,
    /// Replace with bytes that do not decode.
    OnlineChangeMalformed,
    /// Replace with program C, whose retained `Main.r` has another type.
    OnlineChangeIncompatible,
    /// Advance logical time by the given milliseconds.
    AdvanceTime(i64),
}

/// The result of an operation, as the specifications name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    /// A scan on a faulted resource (spec 10 4.7 step 1).
    ResourceFaulted,
    /// A divide by zero in the scan (spec 11 6.6).
    DivisionByZero,
    /// An out-of-range checked write (spec 10 2.4, spec 34 HOST-09).
    SubrangeViolation,
    /// Malformed bytecode (spec 11 6.7.1).
    BytecodeRejected,
    /// A candidate refused for a reason the specifications do not name (spec 11 6.7, 6.7.1).
    CandidateRejected,
}

/// A latched execution fault. `Other` is any fault the model does not expect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    DivisionByZero,
    Other,
}

/// The two retained keys in the store: the global `g_r` and the program variable `@program/Main/r`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub g_r: i16,
    pub r: i16,
}

/// What one modelled operation returns and changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub outcome: Outcome,
    /// True only for an accepted candidate replacement, which installs a new module.
    pub installed_changed: bool,
}

/// Every observable that the model defines. Pending debugger writes are not observable directly.
/// They show up in the values of the next scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub ident: i16,
    pub cycles: u64,
    pub faulted: bool,
    pub last_fault: Option<Fault>,
    pub held: bool,
    pub time_ms: i64,
    pub r: i16,
    pub g_r: i16,
    pub n: i16,
    pub q: i16,
    pub i: i16,
    pub d: i16,
    pub out: bool,
    pub forced_i: Option<i16>,
    pub forced_d: Option<i16>,
    pub store_entries: usize,
    pub store: Option<Checkpoint>,
}

/// Which retained values a restart keeps.
#[derive(Debug, Clone, Copy)]
enum Retained {
    /// The live values (warm restart).
    Live,
    /// The declared initial values (cold restart).
    Initial,
    /// The stored values where the store holds them, otherwise the live values.
    Store,
}

/// The modelled controller state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    exe: Exe,
    fault: Option<Fault>,
    held: bool,
    time_ms: i64,
    cycles: u64,
    r: i16,
    g_r: i16,
    n: i16,
    q: i16,
    i: i16,
    d: i16,
    out: bool,
    pending_i: Option<i16>,
    pending_d: Option<i16>,
    forced_i: Option<i16>,
    forced_d: Option<i16>,
    store: Option<Checkpoint>,
}

impl Model {
    /// Program A installed, nothing executed, no pending or forced writes, empty store.
    #[must_use]
    pub fn initial() -> Self {
        Self {
            exe: Exe::A,
            fault: None,
            held: false,
            time_ms: 0,
            cycles: 0,
            r: R_INIT,
            g_r: G_R_INIT,
            n: N_INIT,
            q: Q_INIT,
            i: I_INIT,
            d: D_INIT,
            out: false,
            pending_i: None,
            pending_d: None,
            forced_i: None,
            forced_d: None,
            store: None,
        }
    }

    /// Whether the specifications define the operation in this state. Other operations are not
    /// generated, because the outcome is open (see the module notes in `stateful_lifecycle_sequences.rs`).
    #[must_use]
    pub fn allows(&self, op: Op) -> bool {
        let healthy = self.fault.is_none();
        match op {
            Op::Scan => !self.held && !self.faulting_scan_after_step_two_writes(),
            Op::RestartWarm
            | Op::RestartCold
            | Op::RestartWarmFromStore
            | Op::OnlineChangeB
            | Op::OnlineChangeMalformed
            | Op::OnlineChangeIncompatible => !self.held,
            Op::WriteI(_)
            | Op::WriteD(_)
            | Op::ForceI(_)
            | Op::ForceD(_)
            | Op::ReleaseI
            | Op::ReleaseD => healthy,
            Op::EnqueueI(_) => healthy && self.pending_i.is_none(),
            Op::EnqueueD(_) => healthy && self.pending_d.is_none(),
            Op::Pause => healthy && !self.held,
            Op::Resume => self.held,
            Op::SaveStore => healthy && !self.held,
            Op::AdvanceTime(_) => true,
        }
    }

    /// The observables that the model defines, for comparison with the product.
    #[must_use]
    pub fn view(&self) -> View {
        View {
            ident: self.exe.ident(),
            cycles: self.cycles,
            faulted: self.fault.is_some(),
            last_fault: self.fault,
            held: self.held,
            time_ms: self.time_ms,
            r: self.r,
            g_r: self.g_r,
            n: self.n,
            q: self.q,
            i: self.i,
            d: self.d,
            out: self.out,
            forced_i: self.forced_i,
            forced_d: self.forced_d,
            store_entries: if self.store.is_some() { 2 } else { 0 },
            store: self.store,
        }
    }

    /// Apply one operation that [`Model::allows`] accepts, and return what the specifications require.
    pub fn step(&mut self, op: Op) -> Step {
        debug_assert!(
            self.allows(op),
            "the driver applies only operations the model defines"
        );
        match op {
            Op::Scan => self.scan(),
            Op::WriteI(value) => {
                self.i = value;
                accepted()
            }
            Op::WriteD(value) => {
                if (0..=10).contains(&value) {
                    self.d = value;
                    accepted()
                } else {
                    refused(Outcome::SubrangeViolation)
                }
            }
            Op::EnqueueI(value) => {
                self.pending_i = Some(value);
                accepted()
            }
            Op::EnqueueD(value) => {
                self.pending_d = Some(value);
                accepted()
            }
            Op::ForceI(value) => {
                self.forced_i = Some(value);
                accepted()
            }
            Op::ForceD(value) => {
                self.forced_d = Some(value);
                accepted()
            }
            Op::ReleaseI => {
                self.forced_i = None;
                accepted()
            }
            Op::ReleaseD => {
                self.forced_d = None;
                accepted()
            }
            Op::Pause => {
                self.held = true;
                accepted()
            }
            Op::Resume => {
                self.held = false;
                accepted()
            }
            Op::RestartWarm => {
                self.restart(Retained::Live);
                accepted()
            }
            Op::RestartCold => {
                self.restart(Retained::Initial);
                accepted()
            }
            Op::RestartWarmFromStore => {
                self.restart(Retained::Store);
                accepted()
            }
            Op::SaveStore => {
                self.store = Some(Checkpoint {
                    g_r: self.g_r,
                    r: self.r,
                });
                accepted()
            }
            Op::OnlineChangeB => {
                self.exe = Exe::B;
                self.restart(Retained::Store);
                Step {
                    outcome: Outcome::Accepted,
                    installed_changed: true,
                }
            }
            Op::OnlineChangeMalformed => refused(Outcome::BytecodeRejected),
            Op::OnlineChangeIncompatible => refused(Outcome::CandidateRejected),
            Op::AdvanceTime(ms) => {
                self.time_ms += ms;
                accepted()
            }
        }
    }

    /// A scan that would fail after it applied queued writes or forces. The specifications name no
    /// rollback for those step-two values after a failed scan (spec 10 4.7, spec 11 6.6), so the driver
    /// does not generate such a scan.
    fn faulting_scan_after_step_two_writes(&self) -> bool {
        let step_two_writes = self.pending_i.is_some()
            || self.pending_d.is_some()
            || self.forced_i.is_some()
            || self.forced_d.is_some();
        // Forces are applied after queued writes, so a force wins.
        let effective_d = self.forced_d.or(self.pending_d).unwrap_or(self.d);
        self.fault.is_none() && step_two_writes && effective_d == 0
    }

    /// One scan (spec 10 4.7, spec 11 6.6, 6.9.2).
    fn scan(&mut self) -> Step {
        if self.fault.is_some() {
            return refused(Outcome::ResourceFaulted);
        }
        // Step 2: queued debugger writes are applied and consumed, then forces are reapplied.
        if let Some(value) = self.pending_i.take() {
            self.i = value;
        }
        if let Some(value) = self.pending_d.take() {
            self.d = value;
        }
        if let Some(value) = self.forced_i {
            self.i = value;
        }
        if let Some(value) = self.forced_d {
            self.d = value;
        }
        // Step 3: the body's first statement divides by d. The fault comes before any body write.
        if self.d == 0 {
            self.fault = Some(Fault::DivisionByZero);
            // Clearing boundary for debugger mutations (spec 11 6.9.2).
            self.forced_i = None;
            self.forced_d = None;
            return refused(Outcome::DivisionByZero);
        }
        let candidate_b = self.exe == Exe::B;
        let threshold = if candidate_b { 4 } else { 10 };
        self.q = 100 / self.d;
        self.r += if candidate_b { 10 } else { 1 };
        self.g_r += if candidate_b { 2 } else { 1 };
        self.n += if candidate_b { self.i * 2 } else { self.i };
        self.out = self.n >= threshold;
        // Step 7: the cycle counter advances only after a complete scan (spec 10 4.7).
        self.cycles += 1;
        accepted()
    }

    /// Warm and cold restart (spec 10 4.8, spec 11 6.7, 6.9.2). The online change of B is the
    /// same transaction with the store as the retained source (spec 11 6.7.1).
    fn restart(&mut self, retained: Retained) {
        // Pending writes and forces are cleared by every restart (spec 11 6.9.2). The fault is cleared
        // because the commit installs fresh state (spec 10 4.8). The cycle count restarts (spec 10 4.8).
        self.fault = None;
        self.pending_i = None;
        self.pending_d = None;
        self.forced_i = None;
        self.forced_d = None;
        self.cycles = 0;
        // Non-retained values are reinitialized (spec 11 6.7). Process outputs are kept (spec 34 RETAIN).
        self.n = N_INIT;
        self.q = Q_INIT;
        self.i = I_INIT;
        self.d = D_INIT;
        match retained {
            Retained::Live => {}
            Retained::Initial => {
                self.r = R_INIT;
                self.g_r = G_R_INIT;
            }
            Retained::Store => {
                if let Some(checkpoint) = self.store {
                    self.r = checkpoint.r;
                    self.g_r = checkpoint.g_r;
                }
            }
        }
    }
}

fn accepted() -> Step {
    Step {
        outcome: Outcome::Accepted,
        installed_changed: false,
    }
}

fn refused(outcome: Outcome) -> Step {
    Step {
        outcome,
        installed_changed: false,
    }
}
