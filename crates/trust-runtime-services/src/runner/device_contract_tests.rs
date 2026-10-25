//! The device settings on the resource loop (specification 34 TARGET-02; specification 11, the device
//! settings contract): `device.get` reports the stored settings without the Wi-Fi password,
//! `device.set` checks and stores them while the resource is Stopped, and `device.reset` resets the
//! device once its reply is sent. A runtime that keeps no device settings answers each with
//! `unsupported request`. Every expected reply is worked out here from the specification.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use serde_json::{json, Value};
use smol_str::SmolStr;

use super::Runner;
use crate::device_settings::DeviceInterfaces;
use crate::events::EVENT_STORE_CAPACITY;
use crate::tests::runner_ports::{ms, Fixture, TestPlatform, TestRunner};

/// The ESP32-C6's interfaces: Wi-Fi and Bluetooth, no Ethernet, a 40 MHz crystal.
const C6: DeviceInterfaces = DeviceInterfaces {
    model: "ESP32-C6",
    wifi: true,
    ethernet: false,
    bluetooth: true,
    crystal_hz: (40_000_000, 40_000_000),
};

/// The passes after which a test gives up waiting for a reply.
const MAX_PASSES: usize = 50;

struct Rig {
    ports: Fixture,
    resets: Rc<Cell<u32>>,
}

impl Rig {
    fn new() -> Self {
        Self {
            ports: Fixture::with_frames_of(8_192),
            resets: Rc::new(Cell::new(0)),
        }
    }

    fn platform(&self, device: bool) -> TestPlatform {
        TestPlatform {
            device: device.then(|| (C6, RefCell::new(None))),
            resets: Rc::clone(&self.resets),
            ..TestPlatform::default()
        }
    }

    /// The Stopped loop of a chip without a program, with device settings unless `device` is false.
    fn stopped(&self, device: bool) -> TestRunner<'_> {
        Runner::stopped(
            self.ports.link.clone(),
            self.ports.wait.clone(),
            self.platform(device),
            self.ports.clock.clone(),
            SmolStr::new_static("no TRSTA002 container in either slot"),
            EVENT_STORE_CAPACITY,
        )
    }

    fn call(&self, runner: &mut TestRunner<'_>, request: &Value) -> Value {
        self.ports.link.send(request);
        for _ in 0..MAX_PASSES {
            runner.run_pass().expect("the test wait does not fail");
            if let Some(reply) = self.ports.link.take_replies().pop() {
                return reply;
            }
        }
        panic!("{request} was not answered");
    }
}

/// Settings stored while Stopped are reported back with their sequence and without the password;
/// a second store takes the next sequence. Catches a password sent back over the link and settings
/// that `device.get` does not report as stored.
#[test]
fn settings_are_stored_while_stopped_and_reported_without_the_password() {
    let rig = Rig::new();
    let mut runner = rig.stopped(true);
    let empty = rig.call(&mut runner, &json!({"id": 1, "type": "device.get"}));
    assert_eq!(
        empty["result"],
        json!({"settings": null, "sequence": null}),
        "{empty}"
    );
    let settings = json!({"device": {"name": "line-3"},
        "network": {"wifi": {"ssid": "Plant", "password": "secret-pass"}}});
    let stored = rig.call(
        &mut runner,
        &json!({"id": 2, "type": "device.set", "params": {"settings": settings}}),
    );
    assert_eq!(
        stored["result"],
        json!({"status": "stored", "sequence": 1, "applies": "boot"}),
        "{stored}"
    );
    let read = rig.call(&mut runner, &json!({"id": 3, "type": "device.get"}));
    assert_eq!(
        read["result"],
        json!({"settings": {"device": {"name": "line-3"},
            "network": {"wifi": {"ssid": "Plant", "password_set": true}},
            "time": {}, "signing": {}, "hardware": {}},
            "sequence": 1}),
        "{read}"
    );
    let again = rig.call(
        &mut runner,
        &json!({"id": 4, "type": "device.set", "params": {"settings": {}}}),
    );
    assert_eq!(again["result"]["sequence"], json!(2));
}

/// Each refusal of `device.set`: settings that are no object, a setting the chip has no hardware
/// for, with the reason, and a resource that is not Stopped. Catches settings stored that the chip
/// cannot carry out, and a flash written while a program runs.
#[test]
fn device_set_refuses_what_the_device_cannot_take() {
    let rig = Rig::new();
    let mut runner = rig.stopped(true);
    let not_object = rig.call(
        &mut runner,
        &json!({"id": 1, "type": "device.set", "params": {"settings": 5}}),
    );
    assert_eq!(
        not_object["error"],
        json!("invalid params: settings must be an object")
    );
    let ethernet = rig.call(
        &mut runner,
        &json!({"id": 2, "type": "device.set",
            "params": {"settings": {"network": {"ethernet": {}}}}}),
    );
    assert_eq!(
        ethernet["error"],
        json!("network.ethernet: the ESP32-C6 has no Ethernet")
    );

    let running_rig = Rig::new();
    let mut running = running_rig.ports.runner_with(running_rig.platform(true));
    running_rig.ports.pass_at(&mut running, 0);
    let refused = running_rig.call(
        &mut running,
        &json!({"id": 3, "type": "device.set", "params": {"settings": {}}}),
    );
    assert_eq!(
        refused["error_code"],
        json!("resource_not_running"),
        "{refused}"
    );
}

/// `device.reset` replies `resetting`, and the device resets only once the reply is sent and 100 ms
/// have passed, answering no request meanwhile. Catches a reset that loses its own reply.
#[test]
fn device_reset_resets_after_its_reply() {
    let rig = Rig::new();
    let mut runner = rig.stopped(true);
    let reply = rig.call(&mut runner, &json!({"id": 1, "type": "device.reset"}));
    assert_eq!(reply["result"], json!({"status": "resetting"}), "{reply}");
    assert_eq!(rig.resets.get(), 0, "not before the grace");
    rig.ports.link.send(&json!({"id": 2, "type": "status"}));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(
        rig.ports.link.take_replies(),
        Vec::<Value>::new(),
        "no request answered"
    );
    rig.ports.clock.advance(ms(100));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(rig.resets.get(), 1);
}

/// A runtime that keeps no device settings, as a computer, answers the three requests with
/// `unsupported request` and never resets. Catches a computer that claims to store settings its
/// operating system holds.
#[test]
fn a_runtime_without_device_settings_refuses_them() {
    let rig = Rig::new();
    let mut runner = rig.stopped(false);
    for (id, kind, params) in [
        (1, "device.get", Value::Null),
        (2, "device.set", json!({"settings": {}})),
        (3, "device.reset", Value::Null),
    ] {
        let reply = rig.call(
            &mut runner,
            &json!({"id": id, "type": kind, "params": params}),
        );
        assert_eq!(
            reply["error"],
            json!("unsupported request"),
            "{kind}: {reply}"
        );
    }
    assert_eq!(rig.resets.get(), 0);
}
