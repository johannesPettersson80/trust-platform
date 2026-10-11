//! Single-owner execution of the unchanged A4 artifacts on real timer samples.
use crate::{memory, profile, trace};
use alloc::boxed::Box;
use core::{cell::Cell, fmt::Write};
use trust_platform_stm32f4::Board;
use trust_runtime_core::{
    value::{Duration, Value},
    vm::{PreparedModule, VmTrap},
};

type Result<T> = core::result::Result<T, &'static str>;

// Finish decoding and heap ownership before recursive instantiation begins.
#[inline(never)]
fn prepare(
    bytes: &[u8],
    console: &mut trust_platform_stm32f4::Console,
    phase: &str,
) -> Result<Box<PreparedModule>> {
    memory::begin_phase();
    let prepared = PreparedModule::from_bytes(bytes, profile::limits()).map_err(|error| {
        trace::failure(console, phase, error.stable_code().as_str());
        "prepare"
    })?;
    let prepared = Box::new(prepared);
    let usage = prepared.preparation_usage();
    writeln!(console, "B1,PREP,{phase},{},{}\r", usage.bytes, usage.work).map_err(|_| "uart")?;
    trace::memory(console, phase)?;
    Ok(prepared)
}

// Keep this phase's temporaries out of the outer runner and other phases.
#[inline(never)]
pub fn main_fixture(board: &mut Board, bytes: &[u8]) -> Result<()> {
    let Board {
        clock,
        io,
        console,
        watchdog,
        ..
    } = board;
    let prepared = prepare(bytes, console, "main-prepare")?;
    memory::begin_phase();
    let services = profile::Services {
        clock,
        deadline: Cell::new(None),
    };
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .map_err(|error| {
            trace::failure(console, "main-instantiate", error.stable_code().as_str());
            "instantiate"
        })?;
    trace::memory(console, "main-instantiate")?;
    let plant = trace::instance(state.storage().get_global("Plant"))?;
    let counter = trace::instance(Some(trace::field(state.storage(), plant, "counter")?))?;
    let timer = trace::instance(Some(trace::field(state.storage(), plant, "delay")?))?;
    memory::begin_phase();
    // A future epoch allows the first sample to be genuinely hardware-timed zero.
    let epoch = clock.micros() + 10_000;
    for scan in 0..=100u64 {
        clock.wait_until_us(epoch + scan * 10_000);
        let started = clock.micros();
        let cycles = clock.cycle_count();
        let now_ms = (started - epoch) / 1000;
        services.deadline.set(Some(started + 10_000));
        state
            .execute_cycle(Duration::from_millis(now_ms as i64))
            .map_err(|error| {
                io.safe_off();
                trace::failure(console, "main-cycle", error.stable_code().as_str());
                "cycle"
            })?;
        let scan_cycles = clock.cycle_count().wrapping_sub(cycles);
        let scan_us = clock.micros() - started;
        watchdog.feed();
        let storage = state.storage();
        let task = state.task_states().first().ok_or("missing-task")?;
        writeln!(
            console,
            "B1,TRACE,{},{},{},{},{},{},{},{},{},{},{}\r",
            now_ms,
            trace::integer(trace::field(storage, counter, "value")?)?,
            trace::integer(trace::field(storage, plant, "result")?)?,
            trace::integer(trace::field(storage, plant, "activations")?)?,
            trace::boolean(trace::field(storage, timer, "Q")?)?,
            trace::integer(trace::field(storage, timer, "ET")?)? / 1_000_000,
            task.last_run.as_millis(),
            task.overrun_count,
            scan_us,
            scan_cycles,
            clock.irq_count()
        )
        .map_err(|_| "uart")?;
    }
    services.deadline.set(None);
    trace::memory(console, "main-run")?;
    io.safe_off();
    trace::io(console, io, "STOP", false)?;
    writeln!(console, "B1,STATE,STOP\r").map_err(|_| "uart")?;
    // Exercise the engine's actual deadline-fault path before safe publication.
    services.deadline.set(Some(clock.micros()));
    let expected = VmTrap::DeadlineExceeded.into_runtime_error();
    match state.execute_cycle(Duration::from_millis(1010)) {
        Err(error) if error == expected && state.fault() == Some(&error) => {}
        _ => return Err("deadline-fault-not-latched"),
    }
    io.safe_off();
    trace::io(console, io, "FAULT", false)?;
    writeln!(console, "B1,STATE,FAULT,{}\r", expected.stable_code()).map_err(|_| "uart")?;
    Ok(())
}

// Keep this phase's temporaries out of the outer runner and other phases.
#[inline(never)]
pub fn numeric_fixture(board: &mut Board, bytes: &[u8]) -> Result<()> {
    let Board {
        clock,
        console,
        watchdog,
        io,
        ..
    } = board;
    let prepared = prepare(bytes, console, "numeric-prepare")?;
    memory::begin_phase();
    let services = profile::Services {
        clock,
        deadline: Cell::new(None),
    };
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .map_err(|error| {
            trace::failure(console, "numeric-instantiate", error.stable_code().as_str());
            "instantiate"
        })?;
    trace::memory(console, "numeric-instantiate")?;
    let plant = trace::instance(state.storage().get_global("Plant"))?;
    memory::begin_phase();
    let epoch = clock.micros() + 10_000;
    let mut max_us = 0;
    let mut max_cycles = 0;
    for scan in 0..=100u64 {
        clock.wait_until_us(epoch + scan * 10_000);
        let started = clock.micros();
        let cycles = clock.cycle_count();
        let now_ms = (started - epoch) / 1000;
        if now_ms != scan * 10 {
            return Err("numeric-sample-late");
        }
        services.deadline.set(Some(started + 10_000));
        state
            .execute_cycle(Duration::from_millis(now_ms as i64))
            .map_err(|error| {
                io.safe_off();
                trace::failure(console, "numeric-cycle", error.stable_code().as_str());
                "cycle"
            })?;
        max_cycles = max_cycles.max(clock.cycle_count().wrapping_sub(cycles));
        max_us = max_us.max(clock.micros() - started);
        watchdog.feed();
    }
    services.deadline.set(None);
    for name in [
        "v_sqrt",
        "v_ln",
        "v_log",
        "v_exp",
        "v_sin",
        "v_cos",
        "v_tan",
        "v_asin",
        "v_acos",
        "v_atan",
        "exact_root",
        "exact_power",
        "sum",
    ] {
        let bits = match trace::field(state.storage(), plant, name)? {
            Value::LReal(value) => value.to_bits(),
            Value::Real(value) => u64::from(value.to_bits()),
            _ => return Err("numeric-type"),
        };
        writeln!(console, "B1,NUM,{name},{bits:016x}\r").map_err(|_| "uart")?;
    }
    let task = state.task_states().first().ok_or("missing-task")?;
    writeln!(
        console,
        "B1,NUM_STATE,{},{},{},{},{},{},{},101\r",
        trace::integer(trace::field(state.storage(), plant, "scaled")?)?,
        trace::boolean(trace::field(state.storage(), plant, "command")?)?,
        trace::integer(trace::field(state.storage(), plant, "activations")?)?,
        max_us,
        max_cycles,
        task.last_run.as_nanos(),
        task.overrun_count
    )
    .map_err(|_| "uart")?;
    trace::memory(console, "numeric-run")?;
    Ok(())
}

// Keep this phase's temporaries out of the outer runner and other phases.
#[inline(never)]
pub fn gpio_fixture(board: &mut Board, bytes: &[u8]) -> Result<()> {
    let Board {
        clock,
        console,
        io,
        watchdog,
        ..
    } = board;
    let prepared = prepare(bytes, console, "gpio-prepare")?;
    memory::begin_phase();
    let services = profile::Services {
        clock,
        deadline: Cell::new(None),
    };
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .map_err(|error| {
            trace::failure(console, "gpio-instantiate", error.stable_code().as_str());
            "instantiate"
        })?;
    trace::memory(console, "gpio-instantiate")?;
    memory::begin_phase();
    let epoch = clock.micros();
    for (index, injected) in [None, Some(false), Some(true), Some(false)]
        .into_iter()
        .enumerate()
    {
        clock.wait_until_us(epoch + (index as u64 + 1) * 10_000);
        let started = clock.micros();
        let now_ms = (started - epoch) / 1000;
        let physical = io.sample();
        let input = injected.unwrap_or(physical.button_pressed);
        let image = state.inputs_mut().first_mut().ok_or("missing-gpio-input")?;
        *image = u8::from(input);
        services.deadline.set(Some(started + 10_000));
        state
            .execute_cycle(Duration::from_millis(now_ms as i64))
            .map_err(|error| {
                io.safe_off();
                trace::failure(console, "gpio-cycle", error.stable_code().as_str());
                "cycle"
            })?;
        let output = *state.outputs().first().ok_or("missing-gpio-output")? & 1 != 0;
        io.set_output(output);
        watchdog.feed();
        writeln!(
            console,
            "B1,GPIO,{},{},{},{},{},{},{}\r",
            if injected.is_some() {
                "injected"
            } else {
                "physical"
            },
            now_ms,
            u8::from(physical.pc13_high),
            u8::from(physical.button_pressed),
            u8::from(input),
            u8::from(output),
            u8::from(io.output_readback())
        )
        .map_err(|_| "uart")?;
    }
    io.safe_off();
    trace::io(console, io, "gpio-STOP", false)?;
    trace::memory(console, "gpio-run")?;
    services.deadline.set(None);
    let plant = trace::instance(state.storage().get_global("Plant"))?;
    for depth in 1..=4i16 {
        state
            .write_global("probe_depth", Value::Int(depth))
            .map_err(|_| "probe-depth")?;
        let previous_deadline = state.task_states().first().ok_or("missing-task")?.last_run;
        let next_deadline_us = u64::try_from(previous_deadline.as_nanos() / 1000)
            .map_err(|_| "negative-task-deadline")?
            .checked_add(10_000)
            .ok_or("task-deadline-overflow")?;
        clock.wait_until_us(
            epoch
                .checked_add(next_deadline_us)
                .ok_or("clock-deadline-overflow")?,
        );
        // Preserve and check prior engineering/logging stack use before repainting.
        let peak = memory::boot_stack_peak();
        trust_nucleo_f401re::stack_guard::check(
            peak,
            trust_nucleo_f401re::stack_guard::STACK_BYTES.saturating_sub(peak),
        )?;
        memory::repaint_inactive_stack();
        let started = clock.micros();
        services.deadline.set(Some(started + 10_000));
        state
            .execute_cycle(Duration::from_millis((started - epoch) as i64 / 1000))
            .map_err(|error| {
                trace::failure(console, "stack-cycle", error.stable_code().as_str());
                "stack-cycle"
            })?;
        let measured = memory::measure();
        if state.task_states().first().ok_or("missing-task")?.last_run <= previous_deadline {
            return Err("stack-task-not-activated");
        }
        let depth_result = trace::integer(trace::field(state.storage(), plant, "depth_result")?)?;
        if depth_result != i64::from(depth) {
            return Err("stack-depth-not-executed");
        }
        writeln!(
            console,
            "B1,STACK,{depth},{},{},{}\r",
            measured.stack_used,
            measured.stack_remaining,
            clock.irq_count()
        )
        .map_err(|_| "uart")?;
        trust_nucleo_f401re::stack_guard::check(measured.stack_used, measured.stack_remaining)?;
        watchdog.feed();
        services.deadline.set(None);
    }
    // Prove STOP and FAULT each turn a previously high physical output off.
    // This uses the core's bound image, not an unrelated direct LED-on write.
    for boundary in ["gpio-STOP-high", "gpio-FAULT"] {
        clock.wait_until_us(clock.micros() + 10_000);
        let started = clock.micros();
        let now_ms = (started - epoch) / 1000;
        services.deadline.set(Some(started + 10_000));
        *state.inputs_mut().first_mut().ok_or("missing-gpio-input")? = 1;
        state
            .execute_cycle(Duration::from_millis(now_ms as i64))
            .map_err(|_| "gpio-high-cycle")?;
        let high = *state.outputs().first().ok_or("missing-gpio-output")? & 1 != 0;
        if !high {
            return Err("gpio-high-not-published");
        }
        io.set_output(high);
        watchdog.feed();
        trace::io(console, io, "gpio-pre-safe", true)?;
        if boundary == "gpio-FAULT" {
            services.deadline.set(Some(clock.micros()));
            let expected = VmTrap::DeadlineExceeded.into_runtime_error();
            if state.execute_cycle(Duration::from_millis(now_ms as i64 + 10))
                != Err(expected.clone())
                || state.fault() != Some(&expected)
            {
                return Err("gpio-fault-not-latched");
            }
        }
        io.safe_off();
        trace::io(console, io, boundary, false)?;
        services.deadline.set(None);
    }
    Ok(())
}
