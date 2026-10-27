//! Identify in the compiled runtime of a computer (specification 11, "Identify"): the drivers get
//! the output image with the identify bit while the blink has it on, the program's own image never
//! holds it, a stop or a fault does not end identify, and an output that the safe state holds keeps
//! its safe value. The program is the board probe: its input `%IX0.0` is copied to its output
//! `%QX0.0`.

use std::sync::{Arc, Mutex};

use trust_runtime_services::identify::IdentifyDuration;

use super::CompiledRuntime;
use crate::config::{IdentifyOutputConfig, IoConfig, IoDriverConfig};
use crate::error::RuntimeError;
use crate::harness::TestHarness;
use crate::io::{host_identify, IdentifyHandle, IoAddress, IoDriver, IoSafeState};
use crate::value::{Duration, Value};

/// The board probe: `Lamp := Probe`, with `Probe` at `%IX0.0` and `Lamp` at `%QX0.0`, so its
/// output image is one byte.
const PROBE: &str = "PROGRAM Main
VAR
    Probe AT %IX0.0 : BOOL;
    Lamp AT %QX0.0 : BOOL;
END_VAR
    Lamp := Probe;
END_PROGRAM
";

/// The images the driver was handed, in order.
type Writes = Arc<Mutex<Vec<Vec<u8>>>>;

struct RecordingDriver(Writes);

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.0
            .lock()
            .expect("recording lock")
            .push(outputs.to_vec());
        Ok(())
    }
}

fn writes(writes: &Writes) -> Vec<Vec<u8>> {
    writes.lock().expect("recording lock").clone()
}

/// The board probe with a recording driver and the input `%IX0.0` on, so the program's output
/// image is `[0x01]`, and `output` as its identify output when there is one.
fn board_probe(output: Option<&str>) -> (CompiledRuntime, IdentifyHandle, Writes) {
    let (harness, handle, recorded) = board_probe_harness(output, Vec::new());
    (harness.into_runtime(), handle, recorded)
}

/// [`board_probe`] in a project whose `io.toml` holds `drivers`.
fn board_probe_in_project(
    output: Option<&str>,
    drivers: Vec<IoDriverConfig>,
) -> (CompiledRuntime, IdentifyHandle, Writes) {
    let (harness, handle, recorded) = board_probe_harness(output, drivers);
    (harness.into_runtime(), handle, recorded)
}

/// [`board_probe_in_project`] in its test harness, which can change the program online.
fn board_probe_harness(
    output: Option<&str>,
    drivers: Vec<IoDriverConfig>,
) -> (TestHarness, IdentifyHandle, Writes) {
    let mut harness = TestHarness::from_source(PROBE).expect("the board probe compiles");
    let runtime = harness.runtime_mut();
    let handle = IdentifyHandle::new();
    runtime.set_identify_handle(handle.clone());
    if let Some(output) = output {
        let setting = IdentifyOutputConfig::parse(output).expect("an identify output");
        let io = IoConfig {
            drivers,
            safe_state: IoSafeState::default(),
        };
        handle.set(Some(
            host_identify(&setting, &io).expect("an output builds"),
        ));
    }
    let recorded = Writes::default();
    runtime.add_io_driver(
        "recording",
        Box::new(RecordingDriver(Arc::clone(&recorded))),
    );
    runtime
        .write_io(
            &IoAddress::parse("%IX0.0").expect("input"),
            Value::Bool(true),
        )
        .expect("the input is in the image");
    runtime.set_current_time(Duration::from_millis(30));
    (harness, handle, recorded)
}

/// The byte of the program's own output image.
fn program_outputs(runtime: &CompiledRuntime) -> u8 {
    match runtime
        .read_io(&IoAddress::parse("%QB0").expect("output byte"))
        .expect("the output image has its byte")
    {
        Value::Byte(byte) => byte,
        other => panic!("a byte, not {other:?}"),
    }
}

/// A safe state that gives `address` the safe value `value`.
fn safe_value(address: &str, value: bool) -> IoSafeState {
    IoSafeState {
        outputs: vec![(
            IoAddress::parse(address).expect("a safe-state address"),
            Value::Bool(value),
        )],
    }
}

fn start_identify(handle: &IdentifyHandle) {
    handle
        .with_identify(|identify| identify.start(IdentifyDuration::new(60).expect("60 s")))
        .expect("the runtime has an identify output")
        .expect("an image bit is always ready");
}

fn is_identifying(handle: &IdentifyHandle) -> bool {
    handle
        .with_identify(|identify| identify.is_identifying())
        .expect("the runtime has an identify output")
}

#[test]
fn the_drivers_get_the_identify_bit_and_the_program_keeps_its_image() {
    // Specification 11, "Identify", the output: after the program's outputs are written, the
    // runtime sets the identify bit in a copy of the output image that it hands to the drivers; the
    // program's own image never holds it. With `output = "%QX0.7"` and the program's output
    // `%QX0.0` on, the driver gets `0x81` (bit 7 and bit 0) and the image of the program stays
    // `0x01`. Catches a bit that is never set, one set in the program's image (where Live Values
    // and the next cycle would see it) and one that replaces the program's own bits.
    let (mut runtime, handle, recorded) = board_probe(Some("%QX0.7"));
    start_identify(&handle);

    runtime.execute_cycle().expect("the cycle runs");

    assert_eq!(writes(&recorded), vec![vec![0x81]]);
    assert_eq!(program_outputs(&runtime), 0x01);
}

#[test]
fn without_identify_running_the_drivers_get_the_image_of_the_program() {
    // Specification 11, "Identify": a runtime that is not identifying hands its drivers exactly
    // the image of the program, whether or not it has an identify output. Catches an identify bit
    // that is set while nothing was asked.
    for output in [None, Some("%QX0.7")] {
        let (mut runtime, _handle, recorded) = board_probe(output);

        runtime.execute_cycle().expect("the cycle runs");

        assert_eq!(writes(&recorded), vec![vec![0x01]], "output={output:?}");
    }
}

#[test]
fn a_stop_hands_the_drivers_the_safe_image_and_identify_goes_on() {
    // Specification 11, "Identify", ending: a stop does not end identify; the safe state of the
    // stop is handed to the drivers without the identify bit, and the stopped wait then blinks the
    // bit on again. With the safe value FALSE for the program's `%QX0.0`, the drivers get `0x81`
    // from the cycle, `0x00` from the safe state, and `0x80` from the first idle pass (the blink
    // is still in its first 250 ms, on); a second idle pass hands nothing. `status` still reports
    // identifying. Catches an identify that the stop ends, one whose bit stays off after the safe
    // image, and an idle pass that hands the image at every pass.
    let (mut runtime, handle, recorded) = board_probe(Some("%QX0.7"));
    runtime.set_io_safe_state(safe_value("%QX0.0", false));
    start_identify(&handle);
    runtime.execute_cycle().expect("the cycle runs");

    runtime
        .apply_io_safe_state()
        .expect("the safe state is applied");
    assert!(is_identifying(&handle), "a stop does not end identify");
    runtime.drive_identify_while_idle();
    runtime.drive_identify_while_idle();

    assert_eq!(writes(&recorded), vec![vec![0x81], vec![0x00], vec![0x80]]);
}

#[test]
fn an_idle_pass_hands_the_committed_image_with_the_bit_once_and_the_end_takes_it_away() {
    // Specification 11, "Identify", the blink and ending: in a pass in which no scan runs, the
    // blink hands the drivers the committed output image with the identify bit when the bit
    // changes, and only then. After one cycle (`0x01`), `identify` starts: the first idle pass
    // turns the bit on and hands `0x81`, the second hands nothing, and the end of the process
    // (`end_identify`) hands the committed `0x01` and ends the run. Catches an idle pass that
    // never reaches the drivers, one that repeats the image at every pass, and an end that leaves
    // the bit on.
    let (mut runtime, handle, recorded) = board_probe(Some("%QX0.7"));
    runtime.execute_cycle().expect("the cycle runs");
    start_identify(&handle);

    runtime.drive_identify_while_idle();
    runtime.drive_identify_while_idle();
    assert_eq!(writes(&recorded), vec![vec![0x01], vec![0x81]]);

    runtime.end_identify();
    assert_eq!(writes(&recorded), vec![vec![0x01], vec![0x81], vec![0x01]]);
    assert!(!is_identifying(&handle));
}

#[test]
fn a_safe_state_output_is_not_blinked_while_faulted() {
    // Specification 11, "Identify", ending: an output that `[[io.safe_state]]` names keeps its
    // safe value while the safe state of a fault is in force, and identify does not blink it then;
    // the run goes on. With the safe value FALSE for `%QX0.7`, the drivers get `0x81` from the
    // cycle and `0x01` from the fault's safe state; an idle pass, which would hand `0x81` for the
    // bit that is on, hands nothing, and `status` still reports identifying. When the program
    // scans again the blink is back: the next cycle hands the bit on. Catches an identify that
    // overrides a safe value, and one that ends or forgets the run at the fault.
    let (mut runtime, handle, recorded) = board_probe(Some("%QX0.7"));
    runtime.set_io_safe_state(safe_value("%QX0.7", false));
    start_identify(&handle);
    runtime.execute_cycle().expect("the cycle runs");

    let _ = runtime.resource_panic("a test fault");
    runtime.drive_identify_while_idle();

    assert_eq!(writes(&recorded), vec![vec![0x81], vec![0x01]]);
    assert!(is_identifying(&handle), "the run goes on");

    runtime
        .restart(crate::RestartMode::Warm)
        .expect("the program restarts");
    runtime.execute_cycle().expect("the cycle runs again");
    let last = writes(&recorded).pop().expect("the cycle handed an image");
    assert_eq!(last[0] & 0x80, 0x80, "the blink is back with the scan");
}

#[test]
fn identify_goes_on_through_a_fault_when_the_safe_state_does_not_name_its_output() {
    // Specification 11, "Identify", ending: a fault does not end identify. With a safe state for
    // the program's `%QX0.0` only (FALSE), the drivers get `0x81` from the cycle and `0x00` from the
    // fault's safe state, and the idle pass that follows hands `0x80`: the output is not named by
    // the safe state, so the blink goes on. Catches an identify that the fault ends, and a safe
    // state that is read as holding every output.
    let (mut runtime, handle, recorded) = board_probe(Some("%QX0.7"));
    runtime.set_io_safe_state(safe_value("%QX0.0", false));
    start_identify(&handle);
    runtime.execute_cycle().expect("the cycle runs");

    let _ = runtime.resource_panic("a test fault");
    runtime.drive_identify_while_idle();

    assert_eq!(writes(&recorded), vec![vec![0x81], vec![0x00], vec![0x80]]);
    assert!(is_identifying(&handle));
}

#[test]
fn a_gpio_identify_line_is_driven_at_the_publication_and_goes_off_when_the_process_ends() {
    // Specification 11, "Identify", the output and ending: a GPIO line is driven directly, not
    // through the image. With `output = "gpio:17"` and a `gpio` driver on the sysfs backend,
    // starting identify makes line 17 an output that starts low, the cycle's publication turns it
    // on, the drivers still get the image of the program unchanged (`0x01`), a stop leaves it
    // blinking (an idle pass of the stopped wait ticks it, still in its first 250 ms: on), and the
    // end of the process drives it off. Catches a line that the publication does not drive, one
    // that stays on when the process ends, a stop that switches it off, and a GPIO identify that
    // also sets a bit of the image.
    let base = std::env::temp_dir().join(format!(
        "trust-runtime-identify-cycle-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let line = base.join("gpio17");
    std::fs::create_dir_all(&line).expect("create the sysfs folder");
    for leaf in ["direction", "value"] {
        std::fs::write(line.join(leaf), "").expect("create a sysfs file");
    }
    let mut params = toml::Table::new();
    params.insert(
        "backend".to_owned(),
        toml::Value::String("sysfs".to_owned()),
    );
    params.insert(
        "sysfs_base".to_owned(),
        toml::Value::String(base.to_string_lossy().into_owned()),
    );
    let gpio = IoDriverConfig {
        name: "gpio".into(),
        params: toml::Value::Table(params),
        enabled: true,
    };
    let value = || std::fs::read_to_string(line.join("value")).expect("the value file");
    let (mut runtime, handle, recorded) = board_probe_in_project(Some("gpio:17"), vec![gpio]);

    start_identify(&handle);
    assert_eq!(value(), "0", "the line is requested low");
    runtime.execute_cycle().expect("the cycle runs");
    assert_eq!(value(), "1", "the publication turns the line on");
    assert_eq!(writes(&recorded), vec![vec![0x01]]);

    runtime
        .apply_io_safe_state()
        .expect("no safe state to apply");
    runtime.drive_identify_while_idle();
    assert_eq!(value(), "1", "a stop leaves the line blinking");
    assert!(is_identifying(&handle));

    runtime.end_identify();
    assert_eq!(value(), "0", "the end of the process drives the line off");
    assert_eq!(
        writes(&recorded),
        vec![vec![0x01]],
        "the drivers were not asked again"
    );
    assert!(!is_identifying(&handle));

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_replacement_program_that_uses_the_identify_output_is_refused_and_the_running_one_stays() {
    // Specification 11, "Identify", load: a program installed while the runtime runs (online
    // change) is checked as a project is at load. The board probe, which binds `%QX0.0`, replaces
    // itself. With the identify output `%QX0.7` the replacement is accepted. With `%QX0.0`, which
    // the replacement binds, it is refused with the output and the binding named, and the running
    // program is untouched: its next cycle runs and hands the drivers its image. Catches an
    // online change that lets the program take over the identify output.
    let (mut harness, _handle, _recorded) = board_probe_harness(Some("%QX0.7"), Vec::new());
    harness
        .reload_source(PROBE)
        .expect("a program that leaves the identify output alone replaces the running one");
    harness
        .runtime_mut()
        .execute_cycle()
        .expect("the replacement runs");

    let (mut harness, _handle, recorded) = board_probe_harness(Some("%QX0.0"), Vec::new());
    let refused = harness
        .reload_source(PROBE)
        .expect_err("a program that binds the identify output is refused");
    assert!(
        refused
            .to_string()
            .contains("identify output %QX0.0 is used by the program: it binds %QX0.0"),
        "{refused}"
    );
    harness
        .runtime_mut()
        .execute_cycle()
        .expect("the running program still runs");
    assert_eq!(writes(&recorded), vec![vec![0x01]]);
}
