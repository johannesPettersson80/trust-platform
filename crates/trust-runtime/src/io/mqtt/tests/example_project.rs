//! The repository's MQTT examples under their own `io.toml`: the MQTT parameters as each example
//! ships them, with only the broker replaced by the broker stand-in.

use std::path::Path;

use super::broker::Broker;
use super::{eventually, start};
use crate::io::IoDriver;

/// The MQTT parameters of the example `examples/communication/<example>` as a table text, its
/// broker replaced by `broker`'s endpoint, and its `topic_in`: `[io.params]` of a single driver, or
/// the `params` of the `mqtt` entry of `io.drivers`.
fn example_params(example: &str, broker: &Broker) -> (String, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/communication")
        .join(example)
        .join("io.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let io: toml::Value = toml::from_str(&text).expect("the example's io.toml");
    let mut params = match io["io"].get("drivers") {
        Some(drivers) => drivers
            .as_array()
            .expect("io.drivers is a list")
            .iter()
            .find(|driver| driver["name"].as_str() == Some("mqtt"))
            .expect("an mqtt driver")["params"]
            .clone(),
        None => io["io"]["params"].clone(),
    };
    let table = params.as_table_mut().expect("the parameters are a table");
    table.insert("broker".into(), toml::Value::String(broker.endpoint()));
    let topic_in = table["topic_in"].as_str().expect("topic_in").to_string();
    (
        toml::to_string(&params).expect("the table's text"),
        topic_in,
    )
}

/// Starts the example's MQTT driver against a broker stand-in, delivers one message to its
/// `topic_in`, and requires that a read with no new message after it succeeds and keeps the input.
fn keeps_its_last_input(example: &str) {
    let broker = Broker::start();
    let (params, topic_in) = example_params(example, &broker);
    let mut driver = start(&params);
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.deliver(&[(topic_in.as_str(), &[1])]);
    let mut inputs = [0_u8; 1];
    eventually("the published byte", || {
        driver.read_inputs(&mut inputs).is_ok() && inputs == [1]
    });
    assert_eq!(
        driver.read_inputs(&mut inputs),
        Ok(()),
        "{example}: a scan with no new message"
    );
    assert_eq!(inputs, [1], "{example}: the last input is kept");
}

#[test]
fn the_mqtt_example_keeps_its_last_input_while_no_new_message_comes() {
    // examples/communication/mqtt/README.md, step 4: the reader publishes one input image byte to
    // `topic_in`, and the runtime keeps running while no new message comes. Specification 32 §8:
    // while the session is connected, a read that finds no new message keeps the input, under
    // every policy. Catches a scan without a new message that fails or loses the input: before
    // G239 the mqtt example's runtime faulted 28 ms after it started ("mqtt input not fresh").
    // The multi_driver example maps a variable instead of `topic_in`; its walkthrough's test in
    // communication_examples_cli.rs runs the whole project and checks the same for its mapping.
    keeps_its_last_input("mqtt");
}
