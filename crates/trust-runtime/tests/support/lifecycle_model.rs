//! Explicit state-transition model for the stateful lifecycle sequences (scope H, checklist A08).
//!
//! The model is written from the specifications, not from the product code. The rules cite spec 10
//! section 4.7 (the scan transaction), section 4.8 (retain, restart and online change), spec 11
//! sections 6.6 (fault, and the state after a faulting scan), 6.7 and 6.7.1 (retain storage,
//! directly represented variables at restart, online change), 6.8.1 (variable mutation contract),
//! 6.9 and 6.9.2 (debugger hold, debug mutation lifecycle, mutations while faulted), and spec 34
//! HOST-09, UPDATE-01, UPDATE-02 and UPDATE-07.
//!
//! Every operation in [`Op`] has a defined outcome in every state, with the exceptions that
//! [`Model::allows`] lists. The one that matters most is a scan under a debugger hold: it waits inside
//! the cycle until the hold is released, so a driver that is also the scan thread cannot issue it, and
//! `debugger_hold_operations.rs` covers it with a second thread.
//!
//! The fixture programs are in `stateful_lifecycle_sequences.rs`. Program A is installed first.
//! Program B is a replacement with the same declarations and other arithmetic; the refused candidate
//! is bytes that do not decode (an online change refuses no candidate because of a value, spec 11
//! 6.7.1). Retained: `g_r` (global) and `Main.r`. Non-retained: `i`, `d`, `n`, `q` and
//! `lamp`. The constant `ident` tells the installed program. `out` is the process output `%QX0.0`,
//! which the program assigns in every scan. `lamp` is the global `lamp AT %QX0.1` with the initial
//! value TRUE; no program assigns it, so its output image `lamp_out` shows how a restart treats the
//! image of a located variable.

const R_INIT: i16 = 1;
const G_R_INIT: i16 = 5;
const N_INIT: i16 = 2;
const Q_INIT: i16 = 0;
const I_INIT: i16 = 0;
const D_INIT: i16 = 1;
const LAMP_INIT: bool = true;

/// The installed application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exe {
    A,
    B,
}

impl Exe {
    /// The value of the program's `ident` constant, which differs between the programs.
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
    /// Checked write of the located output `lamp`.
    WriteLamp(bool),
    /// Queue a debugger write of `i`, applied at the next scan boundary. A second queued write to
    /// `i` replaces the first.
    EnqueueI(i16),
    /// Queue a debugger write of `d`, applied at the next scan boundary. A value outside 0..=10 is
    /// accepted by the queue and refused when the boundary applies it.
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
    /// The variable `lamp`.
    pub lamp: bool,
    /// The image bit `%QX0.1`, which a completed scan sets from `lamp`.
    pub lamp_out: bool,
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
    lamp: bool,
    lamp_out: bool,
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
            lamp: LAMP_INIT,
            lamp_out: false,
            pending_i: None,
            pending_d: None,
            forced_i: None,
            forced_d: None,
            store: None,
        }
    }

    /// Whether the sequence driver issues the operation in this state.
    ///
    /// A scan under a debugger hold waits inside the cycle (spec 11 6.9), so the single-threaded
    /// driver cannot issue it. The control plane refuses pause on a faulted resource (spec 11 6.9),
    /// so the driver pauses only a healthy one. A retain save of a faulted resource is a host choice
    /// that the specifications do not define: the resource's own stop path does not save a faulted
    /// resource, so the driver saves only a healthy one. Every other operation is defined in every
    /// state, including while faulted (spec 11 6.9.2) and while the debugger holds the scan
    /// (spec 11 6.9).
    #[must_use]
    pub fn allows(&self, op: Op) -> bool {
        match op {
            Op::Scan => !self.held,
            Op::Pause => self.fault.is_none() && !self.held,
            Op::Resume => self.held,
            Op::SaveStore => self.fault.is_none(),
            _ => true,
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
            lamp: self.lamp,
            lamp_out: self.lamp_out,
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
            // A checked write is an operation between scans. It does not depend on the resource
            // state, so it is the same while healthy, faulted or held (spec 11 6.9.2, spec 34
            // HOST-09).
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
            Op::WriteLamp(value) => {
                self.lamp = value;
                accepted()
            }
            // The queue accepts a request in every state, and a later request for the same target
            // replaces the queued one in place (spec 11 6.8.1, 6.9.2).
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
            // The restarts and the replacement commit between scans. They clear the queued writes
            // and forces and leave the debugger hold as it was (spec 11 6.9).
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
                self.change();
                Step {
                    outcome: Outcome::Accepted,
                    installed_changed: true,
                }
            }
            Op::OnlineChangeMalformed => refused(Outcome::BytecodeRejected),
            Op::AdvanceTime(ms) => {
                self.time_ms += ms;
                accepted()
            }
        }
    }

    /// One scan (spec 10 4.7, spec 11 6.6, 6.9.2).
    fn scan(&mut self) -> Step {
        if self.fault.is_some() {
            // Step 1 refuses the scan before it touches a queued write or a force (spec 10 4.7).
            return refused(Outcome::ResourceFaulted);
        }
        // Step 2: queued debugger writes are applied in arrival order and consumed. A write is
        // checked when it is applied, so a value its target cannot take is dropped alone (spec 11
        // 6.9.2). Then forces are reapplied, so a force wins over a queued write to the same target.
        if let Some(value) = self.pending_i.take() {
            self.i = value;
        }
        if let Some(value) = self.pending_d.take()
            && (0..=10).contains(&value)
        {
            self.d = value;
        }
        if let Some(value) = self.forced_i {
            self.i = value;
        }
        if let Some(value) = self.forced_d {
            self.d = value;
        }
        // Step 3: the body's first statement divides by d. The fault comes before any body write.
        // The scan is not undone: what step 2 stored stays, the consumed writes are not replayed,
        // and the fault removes the forces without writing a replacement value (spec 11 6.6, state
        // after a faulting scan; 6.9.2). The scan commits no output and does not advance the cycle
        // number.
        if self.d == 0 {
            self.fault = Some(Fault::DivisionByZero);
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
        // Step 6: a completed scan publishes the located variable into its output image (spec 11
        // 6.4 end-of-cycle synchronization).
        self.lamp_out = self.lamp;
        // Step 7: the cycle counter advances only after a complete scan (spec 10 4.7).
        self.cycles += 1;
        accepted()
    }

    /// The commit of an online change (spec 11 6.7.1; spec 34 11.3, decisions OC-1 and OC-2). B
    /// declares what A declares, so every variable keeps its value, retained or not, the value that
    /// faulted the resource included, and no retain store is read. Like a restart, the commit clears
    /// the queued writes, the forces and the fault, restarts the cycle count and leaves the debugger
    /// hold (spec 11 6.6, 6.9, 6.9.2). The process images are carried over.
    fn change(&mut self) {
        self.fault = None;
        self.pending_i = None;
        self.pending_d = None;
        self.forced_i = None;
        self.forced_d = None;
        self.cycles = 0;
    }

    /// Warm and cold restart (spec 10 4.8, spec 11 6.7, 6.9.2).
    fn restart(&mut self, retained: Retained) {
        // Pending writes and forces are cleared by every restart (spec 11 6.9.2), also those that
        // were accepted while the resource was faulted. The fault is cleared because the commit
        // installs fresh state (spec 10 4.8). The cycle count restarts (spec 10 4.8). The debugger
        // hold is not resource state, so it stays (spec 11 6.9; spec 13 reload).
        self.fault = None;
        self.pending_i = None;
        self.pending_d = None;
        self.forced_i = None;
        self.forced_d = None;
        self.cycles = 0;
        // Non-retained values are reinitialized (spec 11 6.7), also the variable that is located at
        // an output: the initializer sets the variable and writes no image byte. Process outputs are
        // carried over (spec 34 10), so `out` and `lamp_out` keep the last published values until a
        // scan completes (spec 11 6.7, directly represented variables at restart).
        self.n = N_INIT;
        self.q = Q_INIT;
        self.i = I_INIT;
        self.d = D_INIT;
        self.lamp = LAMP_INIT;
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
