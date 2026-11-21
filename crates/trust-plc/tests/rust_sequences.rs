//! Child sequences (specification 48 RU-71, RU-72): a sequence awaits another `async fn` of its
//! block, which runs as a call would and keeps a state of its own in `_seq`. Every expected value is
//! worked out by hand from RU-72, activation by activation.

// The expected values are exact, worked out by hand and compared bit for bit.
#![allow(clippy::float_cmp)]

use trust_plc::prelude::*;

/// A filler that waits for a bottle, fills it in a child sequence within 100 ms, counts it and
/// waits a cycle. The child opens the valve until the bottle is full, then lets the level settle
/// for two cycles in a child of its own.
#[trust_program(name = "FILLER", sequence = "run")]
#[budget("60us")]
pub struct Filler {
    #[input]
    pub bottle: bool,
    #[input]
    pub full: bool,
    #[output]
    pub valve: bool,
    #[output]
    pub filled: i16,
    #[output]
    pub active_step: Str<16>,
}

#[trust_sequence]
impl Filler {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.step(&mut self.active_step, "WAIT");
            s.until(|me: &Self| me.bottle).await;
            self.fill(s).within(Time::from_millis(s, 100)?).await?;
            self.filled = checked::add(s, self.filled, 1)?;
            s.step(&mut self.active_step, "DONE");
            s.next_cycle().await;
        }
    }

    async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> {
        s.step(&mut self.active_step, "FILL");
        self.valve = true;
        s.until(|me: &Self| me.full).await;
        self.valve = false;
        self.settle(s).await?;
        Ok(())
    }

    async fn settle(&mut self, s: &mut Seq) -> Result<(), Fault> {
        let mut ticks: i16 = 0;
        while ticks < 2 {
            ticks = checked::add(s, ticks, 1)?;
            s.next_cycle().await;
        }
        Ok(())
    }
}

/// The filler's step, valve and count, where its sequence waits since when, where its children
/// wait, and the settling child's local.
fn filler(f: &Filler) -> (Option<&str>, bool, i16, u16, i64, u16, u16, i16) {
    (
        f.active_step.as_str(),
        f.valve,
        f.filled,
        f._seq.point,
        f._seq.began.as_nanos() / 1_000_000,
        f._seq.fill.point,
        f._seq.settle.point,
        f._seq.settle.ticks,
    )
}

#[test]
fn a_child_sequence_runs_as_a_call_would_and_keeps_a_state_of_its_own() -> Result<(), Fault> {
    // RU-72, every 10 ms. At 0 ms the filler waits for a bottle, point 1. The bottle comes at 10 ms:
    // the same activation starts the child `fill` from its top, which names FILL, opens the valve
    // and waits for the level, its point 1; the filler waits at the child, its point 2, begun at
    // 10 ms. At 30 ms the bottle is full: the child closes the valve and starts `settle`, which
    // counts its local to 1 and waits a cycle; `fill` waits at it, its point 2. At 40 ms `settle`
    // counts to 2 and waits again; at 50 ms it returns, and so do `fill` and the wait of the filler,
    // all at that activation: the filler counts 1, names DONE and waits a cycle, point 3, with its
    // children back at their tops. At 60 ms it waits for a bottle again; at 70 ms the bottle still
    // there starts `fill` again from its top. Catches a child that starts an activation late, a
    // parent that goes on an activation after its child returned, children sharing one state, and
    // a child that resumes where it last stopped.
    let mut cx = trust_plc::test::task(Time::from_nanos(10_000_000));
    let mut f = Filler::new();
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("WAIT"), false, 0, 1, 0, 0, 0, 0),
        "at 0 ms"
    );
    f.bottle = true;
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("FILL"), true, 0, 2, 10, 1, 0, 0),
        "at 10 ms"
    );
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("FILL"), true, 0, 2, 10, 1, 0, 0),
        "at 20 ms"
    );
    f.full = true;
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("FILL"), false, 0, 2, 10, 2, 1, 1),
        "at 30 ms"
    );
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("FILL"), false, 0, 2, 10, 2, 1, 2),
        "at 40 ms"
    );
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("DONE"), false, 1, 3, 50, 0, 0, 2),
        "at 50 ms"
    );
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("WAIT"), false, 1, 1, 60, 0, 0, 2),
        "at 60 ms"
    );
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(
        filler(&f),
        (Some("FILL"), true, 1, 2, 70, 1, 0, 2),
        "at 70 ms"
    );
    Ok(())
}

#[test]
fn a_child_that_outlasts_its_within_faults_with_its_wait_point_and_times() -> Result<(), Fault> {
    // RU-72 and Appendix A: `.within(100 ms)` limits the wait for the child, the filler's point 2,
    // begun at 10 ms when the bottle came. The bottle never fills: at 110 ms the child has run
    // exactly 100 ms, still in time; at 120 ms the activation faults before the child runs on, with
    // RustFault and the numbers [2, 100, 110], and leaves the instance as it was (RU-28): the
    // filler at point 2 since 10 ms, the child waiting for the level, the valve open. Catches a
    // limit counted from the child's own wait, and one checked after the child has run.
    let mut raw = trust_plc_abi::Cx::new(&trust_plc_abi::kernel::WITHOUT_DEBUGGER);
    raw.interval_ns = 10_000_000;
    let mut cx = trust_plc::__private::enter(&raw);
    let mut f = Filler::new();
    f.call(&mut cx)?;
    f.bottle = true;
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    for _ in 0..10 {
        trust_plc::test::advance(&mut cx);
        f.call(&mut cx)?;
    }
    assert_eq!(
        filler(&f),
        (Some("FILL"), true, 0, 2, 10, 1, 0, 0),
        "at 110 ms"
    );
    trust_plc::test::advance(&mut cx);
    let fault = f.call(&mut cx).expect_err("at 120 ms, 110 ms waited");
    let _ = trust_plc::__private::leave(cx, &mut raw);
    assert_eq!(fault.raw(), FaultCode::RustFault as u32);
    assert_eq!(raw.fault.detail, [2, 100, 110]);
    assert_eq!(
        filler(&f),
        (Some("FILL"), true, 0, 2, 10, 1, 0, 0),
        "after the fault"
    );
    Ok(())
}

/// A counter whose child sequence waits a cycle unless told to skip: a child that returns without
/// waiting lets its parent go on at the same activation.
#[trust_fb(name = "SKIPPER", sequence = "run")]
#[budget("20us")]
pub struct Skipper {
    #[input]
    pub skip: bool,
    #[output]
    pub count: i16,
}

#[trust_sequence]
impl Skipper {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            self.maybe(s).await?;
            self.count = checked::add(s, self.count, 1)?;
            s.next_cycle().await;
        }
    }

    async fn maybe(&mut self, s: &mut Seq) -> Result<(), Fault> {
        if self.skip {
            return Ok(());
        }
        s.next_cycle().await;
        Ok(())
    }
}

#[test]
fn a_child_that_returns_without_waiting_lets_its_parent_go_on_at_once() -> Result<(), Fault> {
    // RU-72: with `skip`, the child returns at the activation that started it, so the parent counts
    // at that activation and waits a cycle, point 2: counts 1 and 2 at the first two activations.
    // Without `skip` the child waits a cycle, its point 1, while the parent waits at it, point 1:
    // at activation 3 the parent starts the child, which waits (count 2); at 4 the child returns and
    // the parent counts 3 and waits, point 2; at 5 the child waits again. Catches a child wait point
    // that always takes an activation, and a parent that stays at the child after it returned.
    let mut cx = trust_plc::test::cx();
    let mut c = Skipper::new();
    c.skip = true;
    let mut seen = Vec::new();
    for activation in 1..=5 {
        if activation == 3 {
            c.skip = false;
        }
        c.call(&mut cx)?;
        seen.push((c.count, c._seq.point, c._seq.maybe.point));
    }
    assert_eq!(
        seen,
        [(1, 2, 0), (2, 2, 0), (2, 1, 1), (3, 2, 0), (3, 1, 1)]
    );
    Ok(())
}

/// A shuttle that drives to 10.0 and back to 0.0 for ever, each drive a child sequence that takes
/// its target as a parameter, waits until the shuttle has arrived and names the target it reached.
#[trust_program(name = "SHUTTLE", sequence = "run")]
#[budget("60us")]
pub struct Shuttle {
    #[input]
    pub arrived: bool,
    #[output]
    pub target: f64,
    #[output]
    pub reached: f64,
    #[output]
    pub trips: i16,
}

#[trust_sequence]
impl Shuttle {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            self.go(s, 10.0).await?;
            self.go(s, 0.0).await?;
        }
    }

    async fn go(&mut self, s: &mut Seq, goal: f64) -> Result<(), Fault> {
        self.target = goal;
        s.until(|me: &Self| me.arrived).await;
        self.reached = goal;
        self.trips = checked::add(s, self.trips, 1)?;
        Ok(())
    }
}

#[test]
fn a_childs_parameter_is_set_at_each_await_and_kept_across_its_wait() -> Result<(), Fault> {
    // RU-72: activation 1 awaits go(s, 10.0): the argument sets the child's parameter, `_seq.go.goal`
    // 10.0, the child names the target and waits, its point 1, while the shuttle waits at it,
    // point 1. With `arrived` at activation 3 the child reads `goal` after its wait, 10.0, counts the
    // trip and returns; the same activation awaits go(s, 0.0), which starts the child anew with
    // `goal` 0.0, point 2. At 4, still arrived, it reaches 0.0, and the loop awaits go(s, 10.0) again.
    // Catches an argument evaluated at every activation, a parameter lost across the wait, and a
    // child that keeps the last await's parameter.
    let mut cx = trust_plc::test::cx();
    let mut shuttle = Shuttle::new();
    let mut seen = Vec::new();
    for activation in 1..=5 {
        shuttle.arrived = matches!(activation, 3 | 4);
        shuttle.call(&mut cx)?;
        seen.push((
            shuttle.target,
            shuttle.reached,
            shuttle.trips,
            shuttle._seq.point,
            shuttle._seq.go.goal,
            shuttle._seq.go.point,
        ));
    }
    assert_eq!(
        seen,
        [
            (10.0, 0.0, 0, 1, 10.0, 1),
            (10.0, 0.0, 0, 1, 10.0, 1),
            (0.0, 10.0, 1, 2, 0.0, 1),
            (10.0, 0.0, 2, 1, 10.0, 1),
            (10.0, 0.0, 2, 1, 10.0, 1),
        ]
    );
    Ok(())
}

/// Two stations that work side by side, each a child sequence of several steps, as an SFC's
/// simultaneous sequences: the cell counts a cycle once both have finished.
#[trust_program(name = "TWIN", sequence = "run")]
#[budget("60us")]
// Its inputs and outputs are a PLC's bits, one BOOL each.
#[allow(clippy::struct_excessive_bools)]
pub struct Twin {
    #[input]
    pub left_done: bool,
    #[input]
    pub right_done: bool,
    #[output]
    pub left_on: bool,
    #[output]
    pub right_on: bool,
    #[output]
    pub cycles: i16,
}

#[trust_sequence]
impl Twin {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.all(self.left(s), self.right(s)).await;
            self.cycles = checked::add(s, self.cycles, 1)?;
            s.next_cycle().await;
        }
    }

    async fn left(&mut self, s: &mut Seq) -> Result<(), Fault> {
        self.left_on = true;
        s.until(|me: &Self| me.left_done).await;
        self.left_on = false;
        s.next_cycle().await;
        Ok(())
    }

    async fn right(&mut self, s: &mut Seq) -> Result<(), Fault> {
        self.right_on = true;
        s.until(|me: &Self| me.right_done).await;
        self.right_on = false;
        Ok(())
    }
}

#[test]
fn an_all_of_child_sequences_runs_them_side_by_side() -> Result<(), Fault> {
    // RU-72 and IEC §6.7.5 Table 61 features 4a and 4b. Activation 1 starts both stations in the
    // members' order, each switched on and waiting, its point 1, and the cell waits at the s.all,
    // point 1. At 2 the right station is done and returns: done 0b10. At 3 the left one is done and
    // waits a cycle more, its point 2. At 4 it returns: both done, the cell counts 1 and waits a
    // cycle, point 2, done 0 again. At 5 both stations start anew. Catches children run one after
    // the other, a child resumed after it returned, and an s.all that does not wait for its slower
    // branch.
    let mut cx = trust_plc::test::cx();
    let mut twin = Twin::new();
    let mut seen = Vec::new();
    for activation in 1..=5 {
        twin.right_done = activation >= 2;
        twin.left_done = activation >= 3;
        twin.call(&mut cx)?;
        seen.push((
            twin.left_on,
            twin.right_on,
            twin.cycles,
            twin._seq.point,
            twin._seq.done.0,
            twin._seq.left.point,
            twin._seq.right.point,
        ));
    }
    assert_eq!(
        seen,
        [
            (true, true, 0, 1, 0b00, 1, 1),
            (true, false, 0, 1, 0b10, 1, 0),
            (false, false, 0, 1, 0b10, 2, 0),
            (false, false, 1, 2, 0b00, 0, 0),
            (true, true, 1, 1, 0b00, 1, 1),
        ]
    );
    Ok(())
}

/// A filler whose filling, a child sequence, races a stop: the stop abandons the filling where it
/// waits, and the cell closes the valve itself.
#[trust_program(name = "STOPPABLE", sequence = "run")]
#[budget("60us")]
pub struct Stoppable {
    #[input]
    pub full: bool,
    #[input]
    pub stop: bool,
    #[output]
    pub valve: bool,
    #[output]
    pub ended: u16,
}

#[trust_sequence]
impl Stoppable {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            let won: u16 = s.race(self.fill(s), s.until(|me: &Self| me.stop)).await;
            self.ended = won;
            if won == 2 {
                self.valve = false;
            }
            s.next_cycle().await;
        }
    }

    async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> {
        self.valve = true;
        s.until(|me: &Self| me.full).await;
        self.valve = false;
        Ok(())
    }
}

#[test]
fn a_race_abandons_the_child_that_loses_where_it_waits() -> Result<(), Fault> {
    // RU-72: activation 1 starts the filling, which opens the valve and waits for the level, its
    // point 1. The stop at 2 wins, member 2: the filling is not resumed, keeps its point 1, and the
    // cell closes the valve itself and waits a cycle. At 3 the race begins anew and starts the
    // filling from its top. The level at 4: the filling returns, member 1, and closes the valve.
    // Catches a race that resumes its loser, one that keeps the loser's progress for the next race,
    // and a stop that does not win when it comes first.
    let mut cx = trust_plc::test::cx();
    let mut cell = Stoppable::new();
    let mut seen = Vec::new();
    for activation in 1..=4 {
        cell.stop = activation == 2;
        cell.full = activation == 4;
        cell.call(&mut cx)?;
        seen.push((
            cell.valve,
            cell.ended,
            cell._seq.point,
            cell._seq.fill.point,
            cell._seq.won,
        ));
    }
    assert_eq!(
        seen,
        [
            (true, 0, 1, 1, 0),
            (false, 2, 2, 1, 2),
            (true, 2, 1, 1, 2),
            (false, 1, 2, 0, 1),
        ]
    );
    Ok(())
}

/// A cell whose preparation and check, child sequences, return at once when it is ready, in an
/// s.all and in an s.race.
#[trust_program(name = "READY_CELL", sequence = "run")]
#[budget("60us")]
pub struct ReadyCell {
    #[input]
    pub ready: bool,
    #[input]
    pub go: bool,
    #[output]
    pub prepared: i16,
    #[output]
    pub result: u16,
}

#[trust_sequence]
impl ReadyCell {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.all(self.prepare(s), self.check(s)).await;
            let won: u16 = s.race(s.until(|me: &Self| me.go), self.check(s)).await;
            self.result = won;
            s.next_cycle().await;
        }
    }

    async fn prepare(&mut self, s: &mut Seq) -> Result<(), Fault> {
        self.prepared = checked::add(s, self.prepared, 1)?;
        if self.ready {
            return Ok(());
        }
        s.next_cycle().await;
        Ok(())
    }

    async fn check(&mut self, s: &mut Seq) -> Result<(), Fault> {
        if self.ready {
            return Ok(());
        }
        s.next_cycle().await;
        Ok(())
    }
}

#[test]
fn children_that_return_at_once_complete_their_wait_point_at_once() -> Result<(), Fault> {
    // RU-72: ready, the preparation and the check return at the activation that starts them, so the
    // s.all completes there, and the s.race that follows completes there too, with its child member
    // 2 that returned at once, while `go` could not complete at the activation that began it. So
    // activation 1 prepares once, ends the race with 2 and waits a cycle, point 3; activation 2
    // prepares again and ends the race with 2 again. Catches a child member that takes an activation
    // to complete although it returned, and a race that starts members after the one that won.
    let mut cx = trust_plc::test::cx();
    let mut cell = ReadyCell::new();
    cell.ready = true;
    let mut seen = Vec::new();
    for _ in 1..=2 {
        cell.call(&mut cx)?;
        seen.push((cell.prepared, cell.result, cell._seq.point));
    }
    assert_eq!(seen, [(1, 2, 3), (2, 2, 3)]);
    Ok(())
}
