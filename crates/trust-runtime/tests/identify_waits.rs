//! `identify` in every state of the resource (specification 11, "Identify"). A resource that is
//! paused, stopped or faulted scans nothing, so the scheduler's wait ticks the blink and hands the
//! drivers the committed output image with the identify bit at each edge: the output image bit
//! `%QX0.7`, which the program does not use. The blink is measured on the host's monotonic clock,
//! so these tests run in real time and bound every wait.

mod concurrency_support;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime::config::{IdentifyOutputConfig, IoConfig};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{host_identify, IdentifyHandle, IoAddress, IoDriver, IoSafeState};
use trust_runtime::scheduler::{
    ResourceCommand, ResourceControl, ResourceHandle, ResourceRunner, ResourceState, StdClock,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::FaultPolicy;
use trust_runtime_services::identify::IdentifyDuration;

use concurrency_support::{stop_and_join, wait_until};

/// The program writes `%QX0.0` TRUE in every scan, so its output image is `[0x01]`, and divides by
/// `divisor`, which faults the scan when a test sets it to zero.
const PROGRAM: &str = r#"
VAR_GLOBAL
    lamp AT %QX0.0 : BOOL := FALSE;
    divisor : DINT := DINT#1;
    quotient : DINT := DINT#0;
END_VAR

PROGRAM Main
lamp := TRUE;
quotient := DINT#100 / divisor;
END_PROGRAM
"#;

/// How long a state must hold for a test to call it settled.
const HOLD: WallDuration = WallDuration::from_millis(60);

/// The output images the driver was handed, in order, with the time of each.
type Writes = Arc<Mutex<Vec<(Instant, Vec<u8>)>>>;

struct RecordingDriver(Writes);

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.0
            .lock()
            .expect("recording lock")
            .push((Instant::now(), outputs.to_vec()));
        Ok(())
    }
}

/// A resource thread that scans the program every millisecond, with a recording driver and
/// `%QX0.7` as its identify output.
struct Fixture {
    handle: Option<ResourceHandle<StdClock>>,
    control: ResourceControl<StdClock>,
    identify: IdentifyHandle,
    writes: Writes,
}

impl Fixture {
    /// Starts the resource with `safe_state` as its `[[io.safe_state]]`, and waits for a scan. A
    /// fault hands the drivers the safe state (`safe_halt`).
    fn start(safe_state: IoSafeState) -> Self {
        Self::start_with(safe_state, FaultPolicy::SafeHalt)
    }

    /// [`Self::start`] with the fault policy `policy`: only `safe_halt` hands the drivers the safe
    /// state at a fault, and `halt` freezes them.
    fn start_with(safe_state: IoSafeState, policy: FaultPolicy) -> Self {
        let mut runtime = TestHarness::from_source(PROGRAM)
            .expect("the test program compiles")
            .into_runtime();
        let writes = Writes::default();
        runtime.add_io_driver("recording", Box::new(RecordingDriver(Arc::clone(&writes))));
        runtime.set_io_safe_state(safe_state);
        runtime.set_fault_policy(policy);
        let identify = IdentifyHandle::new();
        let setting = IdentifyOutputConfig::parse("%QX0.7").expect("an identify output");
        let io = IoConfig {
            drivers: Vec::new(),
            safe_state: IoSafeState::default(),
        };
        identify.set(Some(
            host_identify(&setting, &io).expect("an image output builds"),
        ));
        runtime.set_identify_handle(identify.clone());
        let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
            .spawn("identify-waits")
            .expect("spawn the resource");
        let fixture = Self {
            control: handle.control(),
            handle: Some(handle),
            identify,
            writes,
        };
        wait_until("a scan", || !fixture.writes().is_empty());
        fixture
    }

    fn writes(&self) -> Vec<(Instant, Vec<u8>)> {
        self.writes.lock().expect("recording lock").clone()
    }

    /// The images handed to the driver from the write number `from` on.
    fn images_from(&self, from: usize) -> Vec<Vec<u8>> {
        self.writes()[from..]
            .iter()
            .map(|(_, image)| image.clone())
            .collect()
    }

    fn start_identify(&self, seconds: u64) {
        self.identify
            .with_identify(|identify| {
                identify.start(IdentifyDuration::new(seconds).expect("1 to 60 s"))
            })
            .expect("the runtime has an identify output")
            .expect("an image bit is always ready");
    }

    fn identifying(&self) -> bool {
        self.identify
            .with_identify(|identify| identify.is_identifying())
            .expect("the runtime has an identify output")
    }

    /// Stops the program and waits until the resource is `Stopped`; the last scan's writes are in.
    fn stop(&self) {
        self.control.stop_program();
        self.wait_for_state(ResourceState::Stopped);
        thread::sleep(HOLD);
    }

    fn pause(&self) {
        self.control.pause().expect("command channel open");
        self.wait_for_state(ResourceState::Paused);
        thread::sleep(HOLD);
    }

    /// Makes the next scan divide by zero and waits until the resource is `Faulted`.
    fn fault(&self) {
        let mut updates = IndexMap::new();
        updates.insert(SmolStr::new("divisor"), Value::DInt(0));
        self.control
            .send_command(ResourceCommand::MeshApply {
                updates,
                source: None,
                sequence: None,
            })
            .expect("command channel open");
        self.wait_for_state(ResourceState::Faulted);
        thread::sleep(HOLD);
    }

    fn wait_for_state(&self, state: ResourceState) {
        wait_until(&format!("the {state:?} state"), || {
            self.control.state() == state
        });
    }

    fn finish(mut self) {
        stop_and_join(self.handle.take().expect("the resource thread"));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.stop();
        }
    }
}

/// A safe state that gives `address` the safe value FALSE.
fn safe_false(address: &str) -> IoSafeState {
    IoSafeState {
        outputs: vec![(
            IoAddress::parse(address).expect("a safe-state address"),
            Value::Bool(false),
        )],
    }
}

/// Asserts that the writes from `from` on are `[0x81], [0x01], [0x81], [0x01]` for a run of one
/// second: 250 ms on, 250 ms off, twice, then nothing, and the run is over.
fn assert_one_second_of_blink(fixture: &Fixture, from: usize) {
    wait_until("four edges of the blink", || {
        fixture.writes().len() >= from + 4
    });
    // The second is over at 1000 ms and the output is already off: nothing more is handed.
    thread::sleep(WallDuration::from_millis(450));
    let writes = fixture.writes();
    assert_eq!(
        fixture.images_from(from),
        vec![vec![0x81], vec![0x01], vec![0x81], vec![0x01]],
        "250 ms on and 250 ms off, twice, at the committed image"
    );
    for pair in writes[from..].windows(2) {
        let gap = pair[1].0.duration_since(pair[0].0);
        assert!(
            gap >= WallDuration::from_millis(240),
            "an edge came {gap:?} after the one before it"
        );
        assert!(
            gap < WallDuration::from_millis(600),
            "an edge came {gap:?} after the one before it"
        );
    }
    assert!(!fixture.identifying(), "the second is over");
}

#[test]
fn identify_blinks_while_the_resource_is_stopped() {
    // Specification 11, "Identify", the blink: a stopped resource scans nothing, and the stopped
    // wait ticks identify (every ~10 ms) and hands the drivers the committed image with the bit.
    // A request of 1 s on the stopped resource turns `%QX0.7` on at once, off 250 ms later, on at
    // 500 ms and off at 750 ms, and the second is over at 1000 ms with the output off: the drivers
    // get `0x81`, `0x01`, `0x81`, `0x01` (the program's `%QX0.0` stays on) and nothing more. Wrong
    // behaviour: a stopped wait that does not tick (no write comes), a blink that repeats the image
    // at every poll, or one that does not end with the duration.
    let fixture = Fixture::start(IoSafeState::default());
    fixture.stop();
    let before = fixture.writes().len();

    fixture.start_identify(1);

    assert_one_second_of_blink(&fixture, before);
    fixture.finish();
}

#[test]
fn identify_blinks_while_the_resource_is_paused() {
    // Specification 11, "Identify", the blink: a paused resource scans nothing and its outputs
    // keep their committed values, and the paused wait ticks identify. The same request of 1 s
    // hands the drivers `0x81`, `0x01`, `0x81`, `0x01`: the program's bit stays on and only the
    // identify bit changes. Wrong behaviour: a paused wait that leaves the blink to the scan, which
    // is not running.
    let fixture = Fixture::start(IoSafeState::default());
    fixture.pause();
    let before = fixture.writes().len();

    fixture.start_identify(1);

    assert_one_second_of_blink(&fixture, before);
    fixture.finish();
}

#[test]
fn identify_blinks_while_the_resource_is_faulted() {
    // Specification 11, "Identify", the blink: a faulted resource scans nothing, and identify blinks
    // an output that the safe state does not hold. After a divide-by-zero fault the same request of
    // 1 s hands the drivers `0x81`, `0x01`, `0x81`, `0x01`. Wrong behaviour: a faulted wait that
    // does not tick, which would leave a faulted box impossible to find.
    let fixture = Fixture::start(IoSafeState::default());
    fixture.fault();
    let before = fixture.writes().len();

    fixture.start_identify(1);

    assert_one_second_of_blink(&fixture, before);
    fixture.finish();
}

#[test]
fn identify_goes_on_through_a_fault_when_the_safe_state_does_not_name_its_output() {
    // Specification 11, "Identify", ending: a fault does not end identify. A run of 60 s that the
    // scan is blinking continues in the faulted wait, with the image of the fault's safe state
    // (`%QX0.0` FALSE, so the program's byte is `0x00`): after the safe image `0x00` the drivers get
    // `0x80`, `0x00`, `0x80`, `0x00` in turn, always starting with the bit on, and the run is still
    // going afterwards. Wrong behaviour: an identify that the fault ends or that stops blinking.
    let fixture = Fixture::start(safe_false("%QX0.0"));
    fixture.start_identify(60);
    wait_until("the bit on in a scan", || {
        fixture
            .writes()
            .iter()
            .any(|(_, image)| image[0] & 0x80 != 0)
    });
    fixture.fault();

    thread::sleep(WallDuration::from_millis(900));

    // The scans wrote `0x01` and `0x81`; the first `0x00` is the fault's safe image, and what
    // follows is the blink of the faulted wait.
    let images = fixture.images_from(0);
    let safe_image = images
        .iter()
        .position(|image| image == &vec![0x00])
        .expect("the fault handed the safe image");
    let blink = &images[safe_image + 1..];
    assert!(
        blink.len() >= 2,
        "the blink goes on in the faulted wait: {blink:?}"
    );
    for (position, image) in blink.iter().enumerate() {
        let expected = if position % 2 == 0 { 0x80 } else { 0x00 };
        assert_eq!(image, &vec![expected], "write {position} of {blink:?}");
    }
    assert!(fixture.identifying(), "a fault does not end identify");
    fixture.finish();
}

#[test]
fn a_safe_state_output_is_not_blinked_while_faulted() {
    // Specification 11, "Identify", ending: an output that `[[io.safe_state]]` names keeps its safe
    // value while faulted, and identify does not blink it then. `%QX0.7` is the identify output
    // and has the safe value FALSE. The fault hands the drivers the safe image `0x01`; during the
    // 900 ms after it, which hold three edges of the blink and an on phase, nothing more is handed,
    // and the run of 60 s is still going. Wrong behaviour: an identify that overrides the safe
    // value of its output, which would hand `0x81` in an on phase.
    let fixture = Fixture::start(safe_false("%QX0.7"));
    fixture.start_identify(60);
    wait_until("the bit on in a scan", || {
        fixture
            .writes()
            .iter()
            .any(|(_, image)| image[0] & 0x80 != 0)
    });
    fixture.fault();
    let safe_image = fixture
        .writes()
        .last()
        .map(|(_, image)| image.clone())
        .expect("the fault handed the safe image");
    assert_eq!(safe_image, vec![0x01], "the safe image has the bit off");
    let writes_at_the_fault = fixture.writes().len();

    thread::sleep(WallDuration::from_millis(900));

    assert_eq!(
        fixture.images_from(writes_at_the_fault),
        Vec::<Vec<u8>>::new(),
        "the output keeps its safe value"
    );
    assert!(fixture.identifying(), "the run goes on");
    fixture.finish();
}

#[test]
fn a_halting_fault_leaves_the_output_to_identify_because_no_safe_state_is_in_force() {
    // Specification 11, "Identify", ending: an output keeps its safe value only while the safe state
    // of a stop or a fault is in force. The fault policy `halt` hands the drivers no safe state, so
    // the outputs stay as the last scan left them, and `[[io.safe_state]]` naming `%QX0.7` holds
    // nothing: identify blinks it in the faulted wait. After the fault the bit changes at each edge
    // (opposite to the last image handed out first), at least twice in 900 ms. Wrong behaviour: a
    // blink that is held off by a safe state that was never handed to the drivers.
    let fixture = Fixture::start_with(safe_false("%QX0.7"), FaultPolicy::Halt);
    fixture.start_identify(60);
    wait_until("the bit on in a scan", || {
        fixture
            .writes()
            .iter()
            .any(|(_, image)| image[0] & 0x80 != 0)
    });
    fixture.fault();
    let at_the_fault = fixture.writes().len();
    let last_handed_out = fixture.images_from(0)[at_the_fault - 1][0] & 0x80;

    thread::sleep(WallDuration::from_millis(900));

    let blink = fixture.images_from(at_the_fault);
    assert!(
        blink.len() >= 2,
        "the blink goes on in the faulted wait: {blink:?}"
    );
    for (position, image) in blink.iter().enumerate() {
        let flipped = if position % 2 == 0 {
            last_handed_out ^ 0x80
        } else {
            last_handed_out
        };
        assert_eq!(
            image,
            &vec![0x01 | flipped],
            "write {position} of {blink:?}"
        );
    }
    fixture.finish();
}
