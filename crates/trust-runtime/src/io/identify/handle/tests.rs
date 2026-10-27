//! The shared identify state (specification 11, "Identify"): what the scan and the waits of a
//! resource that scans nothing are offered, and that neither of them waits for a control request.
//! Time is the host's monotonic clock, so these tests stay within the first 250 ms of a run,
//! where the blink is on, and never wait.

use trust_runtime_services::identify::IdentifyDuration;

use super::*;
use crate::config::{IdentifyOutputConfig, IoConfig};
use crate::io::IoSafeState;

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("a valid address")
}

/// A handle whose identify output is the image bit `%QX0.7`.
fn image_handle() -> IdentifyHandle {
    let handle = IdentifyHandle::new();
    let setting = IdentifyOutputConfig::Address(address("%QX0.7"));
    let io = IoConfig {
        drivers: Vec::new(),
        safe_state: IoSafeState::default(),
    };
    handle.set(Some(
        crate::io::host_identify(&setting, &io).expect("an image output builds"),
    ));
    handle
}

fn start(handle: &IdentifyHandle) {
    handle
        .with_identify(|identify| identify.start(IdentifyDuration::new(60).expect("60 s")))
        .expect("the handle has an identify state")
        .expect("an image bit is always ready");
}

fn bit_7() -> ImageBit {
    ImageBit::of(&address("%QX0.7")).expect("one flat bit")
}

fn never_held(_: &IoAddress) -> bool {
    false
}

#[test]
fn a_handle_without_an_identify_state_offers_nothing_and_answers_none() {
    // Specification 11, "Identify": a runtime without `[runtime.identify] output` has no identify
    // state: the request finds none, the scan is offered no bit, an idle pass hands nothing and
    // nothing runs. Catches a handle that invents a state, and a scan that is offered a bit.
    let handle = IdentifyHandle::new();

    assert!(handle.with_identify(|_| ()).is_none());
    assert_eq!(handle.publication(), None);
    assert_eq!(handle.idle_publication(never_held), IdleOutput::Unchanged);
    assert!(!handle.is_identifying());
    assert_eq!(handle.image_address(), None);
    assert!(!handle.end());
}

#[test]
fn the_scan_is_offered_the_bit_while_the_blink_has_it_on() {
    // Specification 11, "Identify", the blink: the first point after the request turns the output
    // on. After a request, the scan's publication is offered the bit `%QX0.7`; before it, none.
    // Catches a publication that ignores the run, and one that never offers the bit.
    let handle = image_handle();
    assert_eq!(handle.publication(), None, "nothing was asked yet");

    start(&handle);

    assert_eq!(handle.publication(), Some(bit_7()));
    assert!(handle.is_identifying());
    assert_eq!(handle.image_address(), Some(address("%QX0.7")));
}

#[test]
fn an_idle_pass_hands_the_image_at_an_edge_and_not_at_every_pass() {
    // Specification 11, "Identify", the blink: while no scan runs the committed image is handed to
    // the drivers when the bit changes. The first pass after the request turns the bit on and hands
    // the image with it; the second pass, still within the first 250 ms, hands nothing. Catches an
    // idle pass that writes to the drivers at every poll, and one that never writes.
    let handle = image_handle();
    start(&handle);

    assert_eq!(
        handle.idle_publication(never_held),
        IdleOutput::Hand(Some(bit_7()))
    );
    assert_eq!(handle.idle_publication(never_held), IdleOutput::Unchanged);
}

#[test]
fn a_safe_value_holds_the_bit_off_and_the_run_goes_on() {
    // Specification 11, "Identify", ending: an output that the safe state names keeps its safe
    // value, and identify does not blink it. The idle pass that is told the address is held offers
    // no bit and the run goes on; the pass that is not told so hands the bit. Catches a blink that
    // overrides a safe value and a run that ends with it.
    let handle = image_handle();
    start(&handle);

    let held = handle.idle_publication(|address| address == &self::address("%QX0.7"));

    assert_eq!(held, IdleOutput::Unchanged);
    assert!(handle.is_identifying(), "the run goes on");
    assert_eq!(
        handle.idle_publication(never_held),
        IdleOutput::Hand(Some(bit_7())),
        "without the safe value the blink is back"
    );
}

#[test]
fn drivers_that_got_the_image_without_the_bit_are_handed_it_again() {
    // Specification 11, "Identify", ending: the safe image of a stop has no identify bit, and the
    // blink goes on in the stopped wait. After a scan offered the bit (the drivers hold it), the
    // safe image is handed out and recorded; the next idle pass hands the image with the bit
    // again. Catches a blink that believes the drivers still hold a bit that the safe image took
    // away.
    let handle = image_handle();
    start(&handle);
    assert_eq!(handle.publication(), Some(bit_7()));
    assert_eq!(handle.idle_publication(never_held), IdleOutput::Unchanged);

    handle.image_handed_out_without_bit();

    assert_eq!(
        handle.idle_publication(never_held),
        IdleOutput::Hand(Some(bit_7()))
    );
}

#[test]
fn neither_the_scan_nor_an_idle_pass_waits_for_a_request() {
    // Specification 11, "Identify": the resource thread never waits for a control request. While
    // a request holds the identify state, the scan's publication offers no bit and an idle pass
    // hands nothing, instead of waiting; afterwards they work again. Catches a tick that blocks
    // the scan or a wait behind a request.
    let handle = image_handle();
    start(&handle);

    let (publication, idle) = handle
        .with_identify(|_| {
            let other = handle.clone();
            (other.publication(), other.idle_publication(never_held))
        })
        .expect("the handle has an identify state");

    assert_eq!(publication, None);
    assert_eq!(idle, IdleOutput::Unchanged);
    assert_eq!(handle.publication(), Some(bit_7()));
}

#[test]
fn end_drives_the_output_off_and_reports_whether_the_drivers_hold_the_bit() {
    // Specification 11, "Identify", ending: the end of the process ends identify. After a scan
    // offered the bit, `end` reports that the drivers hold it, so the caller hands them the
    // committed image; a second `end` reports nothing to take away. Catches an end that leaves a
    // bit on in the drivers' image and one that reports it twice.
    let handle = image_handle();
    start(&handle);
    assert_eq!(handle.publication(), Some(bit_7()));

    assert!(handle.end(), "the drivers hold the bit");
    assert!(!handle.end(), "there is nothing left to take away");
    assert!(!handle.is_identifying());
}

#[test]
fn setting_a_new_state_ends_the_run_of_the_one_it_replaces() {
    // Specification 11, "Identify": a restart that reloads the program reads `[runtime.identify]`
    // again; the run in progress ends and the setting of the project now is in force. Replacing
    // the state by none answers `identify_not_configured` again (the handle has no state); a
    // new state starts with no run. Catches a run that outlives the program it was started on.
    let handle = image_handle();
    start(&handle);
    assert!(handle.is_identifying());

    handle.set(None);
    assert!(handle.with_identify(|_| ()).is_none());
    assert!(!handle.is_identifying());

    let replaced = image_handle();
    start(&replaced);
    let setting = IdentifyOutputConfig::Address(address("%QX0.6"));
    let io = IoConfig {
        drivers: Vec::new(),
        safe_state: IoSafeState::default(),
    };
    replaced.set(Some(
        crate::io::host_identify(&setting, &io).expect("an image output builds"),
    ));
    assert!(!replaced.is_identifying(), "the new state has no run");
    assert_eq!(replaced.image_address(), Some(address("%QX0.6")));
}
