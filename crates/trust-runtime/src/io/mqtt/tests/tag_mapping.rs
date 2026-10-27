//! The presence of `mappings` and the directions it selects (specification 32 §4 and §5), with the
//! broker stand-in as the evidence of what the driver subscribes to and publishes. The runtime
//! lowers each mapping to a typed point before the driver starts (`io::mqtt_tag_mapping`); the
//! driver takes the lowered table, which these tests give it as the runtime does. The lowering
//! itself is proven with programs by the launcher's tests
//! (`bin/trust-runtime/run/tests/mqtt_tag_mapping.rs`).

use std::thread;
use std::time::Duration;

use smol_str::SmolStr;

use super::broker::Broker;
use super::{at, read, read_fresh, refused, start};
use crate::io::{IoDriver, IoDriverHealth};

/// How long a test gives a driver that must not connect to show that it does not.
const QUIET: Duration = Duration::from_millis(50);

/// Write-only mappings select mapped mode (specification 32 §4): the driver subscribes to nothing,
/// so no `trust/io/in`, needs no input snapshot and leaves the input bytes of other drivers; as
/// written, without their points, they exchange nothing and the driver never connects, and lowered
/// to their output point they publish it, with no SUBSCRIBE before (specification 32 §10: the
/// broker's evidence shows no subscription to `trust/io/in`). Catches the raw input fallback of a
/// write-only mapping, which the specification calls a configuration-mode defect.
#[test]
fn write_only_tag_mappings_disable_raw_input_subscription_and_freshness_wait() {
    let mappings = "mappings = [\n  { tag = \"MainInstance.Green\", topic = \
                    \"traffic/north/green\", direction = \"write\" }\n]\n";
    let written =
        read(&format!("broker = \"127.0.0.1:1883\"\n{mappings}")).expect("write-only mappings");
    assert_eq!(written.subscribe_topics(), Vec::<SmolStr>::new());
    let broker = Broker::start();
    let mut unlowered = start(&at(&broker, mappings));
    let mut inputs = [0xA5];
    assert_eq!(unlowered.read_inputs(&mut inputs), Ok(()));
    assert_eq!(inputs, [0xA5]);
    thread::sleep(QUIET);
    assert_eq!(broker.record(|record| record.accepted), 0);

    let lowered_broker = Broker::start();
    let mut lowered = start(&at(
        &lowered_broker,
        &format!(
            "{mappings}output_points = [\n  {{ topic = \"traffic/north/green\", image_offset = 0, \
             image_bit = 0, data_type = \"bool\" }}\n]\n"
        ),
    ));
    super::eventually("session in the output path", || {
        lowered.output_health() == IoDriverHealth::Ok
    });
    assert_eq!(lowered.read_inputs(&mut inputs), Ok(()));
    assert_eq!(inputs, [0xA5]);
    assert_eq!(lowered.write_outputs(&[1]), Ok(()));
    lowered_broker.wait_for("green", |record| !record.publishes.is_empty());
    lowered_broker.record(|record| {
        assert_eq!(record.published("traffic/north/green"), [b"true".to_vec()]);
        assert!(
            record.subscriptions.is_empty(),
            "{:?}",
            record.subscriptions
        );
    });
}

/// Read-only mappings publish neither a mapped output nor the raw output image (specification 32
/// §4): lowered to their input point, the driver subscribes to the point's topic alone, reads it,
/// and a write publishes nothing; its output path, a direction it does not have, is `Ok`. Catches
/// the process image published to `trust/io/out` by a driver that only reads.
#[test]
fn read_only_tag_mappings_disable_raw_output_publication() {
    let mappings = "mappings = [\n  { tag = \"MainInstance.Command\", topic = \
                    \"traffic/north/command\", direction = \"read\" }\n]\n";
    let written =
        read(&format!("broker = \"127.0.0.1:1883\"\n{mappings}")).expect("read-only mappings");
    assert!(!written.output_enabled());
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        &format!(
            "{mappings}input_points = [\n  {{ topic = \"traffic/north/command\", \
             image_offset = 0, image_bit = 0, data_type = \"bool\" }}\n]\n"
        ),
    ));
    broker.wait_for("subscription", |record| {
        record.subscriptions == ["traffic/north/command"]
    });
    assert_eq!(driver.write_outputs(&[0xA5]), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    broker.deliver(&[("traffic/north/command", b"true")]);
    let mut inputs = [0_u8];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [1]);
    assert!(broker.record(|record| record.publishes.is_empty()));
}

/// An explicitly empty mapping list turns both raw directions off (specification 32 §4): the
/// driver subscribes to nothing, publishes nothing, leaves the input image and never connects, and
/// its health is `Ok`, as a driver with nothing to exchange. Catches `mappings = []` read as
/// absent.
#[test]
fn explicit_empty_tag_mappings_disable_both_raw_directions() {
    let empty = read("broker = \"127.0.0.1:1883\"\nmappings = []\n").expect("empty mappings");
    assert_eq!(empty.subscribe_topics(), Vec::<SmolStr>::new());
    assert!(!empty.input_enabled() && !empty.output_enabled());
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "mappings = []\n"));
    let mut inputs = [0xA5];
    assert_eq!(driver.read_inputs(&mut inputs), Ok(()));
    assert_eq!(driver.write_outputs(&[0x5A]), Ok(()));
    assert_eq!(inputs, [0xA5]);
    thread::sleep(QUIET);
    assert_eq!(broker.record(|record| record.accepted), 0);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// With an empty mapping list, explicitly configured points stay active without the raw fallback
/// (specification 32 §4): the output point is published, and nothing is subscribed to. Catches
/// explicit points switched off by the mapped mode.
#[test]
fn explicit_empty_tag_mappings_keep_explicit_points_active_without_raw_fallback() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        "mappings = []\noutput_points = [\n  { topic = \"traffic/north/green\", image_offset = 0, \
         image_bit = 0, data_type = \"bool\", payload_format = \"text\" }\n]\n",
    ));
    super::eventually("session in the output path", || {
        driver.output_health() == IoDriverHealth::Ok
    });
    assert_eq!(driver.write_outputs(&[1]), Ok(()));
    broker.wait_for("green", |record| !record.publishes.is_empty());
    broker.record(|record| {
        assert_eq!(record.topics(), ["traffic/north/green"]);
        assert_eq!(record.published("traffic/north/green"), [b"true".to_vec()]);
        assert!(record.subscriptions.is_empty());
    });
}

/// Mixed mappings, lowered, use only their points' topics (specification 32 §4): the driver
/// subscribes to the read mapping's topic, reads it, and publishes the write mapping's point alone,
/// never `trust/io/out`. Catches a raw fallback beside the mapped points.
#[test]
fn mixed_tag_mappings_use_only_the_lowered_point_topics() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        r#"mappings = [
  { tag = "MainInstance.Command", topic = "traffic/north/command", direction = "read" },
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" }
]
input_points = [
  { topic = "traffic/north/command", image_offset = 0, image_bit = 0, data_type = "bool", payload_format = "text" }
]
output_points = [
  { topic = "traffic/north/green", image_offset = 0, image_bit = 0, data_type = "bool", payload_format = "text" }
]
"#,
    ));
    broker.wait_for("subscription", |record| {
        record.subscriptions == ["traffic/north/command"]
    });
    broker.deliver(&[("traffic/north/command", b"true")]);
    let mut inputs = [0_u8];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [1]);
    assert_eq!(driver.write_outputs(&[1]), Ok(()));
    broker.wait_for("green", |record| !record.publishes.is_empty());
    assert_eq!(
        broker.record(|record| record.topics().join(",")),
        "traffic/north/green"
    );
}

/// An unknown key of the table is refused (specification 32 §2: unknown fields are configuration
/// errors, not ignored), here `mapppings`, which would otherwise leave the driver in raw mode. The
/// shared rules' text replaces the computer's (`unknown field `mapppings``). Catches a misspelt
/// `mappings` that turns the raw directions on.
#[test]
fn mqtt_config_rejects_unknown_top_level_parameter() {
    assert_eq!(
        read("broker = \"127.0.0.1:1883\"\nmapppings = []\n"),
        refused(
            "mapppings",
            "unknown key; the mqtt driver takes allow_insecure_remote, broker, client_id, \
             input_points, keep_alive_s, mappings, on_error, output_points, password, \
             reconnect_ms, sparkplug, tls, tls_alpn, tls_ca_path, tls_client_cert_path, \
             tls_client_key_path, topic_in, topic_out and username"
        )
    );
}
