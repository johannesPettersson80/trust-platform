//! The scan side of the `mqtt` driver's exchange (specification 32 §4, §7 and §8; specification
//! 11, deadline integrity and safe-state handoff confirmation): the input read at the cycle start,
//! the output handoff after the tasks, and the driver's health on its two paths.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;

use super::{publish_packet, Link, Message, MqttExchange, Snapshot};
use crate::io_drivers::mqtt::payload::{self, ImageValue};
use crate::io_drivers::mqtt::points::InputPoint;
use crate::io_drivers::mqtt::sparkplug;
use crate::io_drivers::{quoted, OnError};
use crate::status::IoDriverHealth;

/// The input condition of a read that finds no message since the last read.
const NOT_FRESH: &str = "mqtt input not fresh";

/// The input condition of a degraded read without an accepted snapshot to copy.
const SNAPSHOT_UNAVAILABLE: &str = "mqtt input snapshot unavailable";

/// The publish condition of a handoff that waits for an earlier one.
const HANDOFF_PENDING: &str = "mqtt output handoff pending";

impl MqttExchange {
    /// Reads the inputs into `image`, the input image, at `now_ms`, without waiting
    /// (specification 32 §4 and §8). Every message since the last read is one batch: in raw mode
    /// the newest payload of `topic_in` fills the image, zero past its end; with typed points each
    /// message converts to its point, and a batch with one failure changes neither the snapshot
    /// nor the image. Bytes of the image that no point uses are left as they are. A driver without
    /// inputs leaves the image as it is.
    ///
    /// # Errors
    ///
    /// Under `on_error = "fault"`, with the image unchanged: [`RuntimeError::IoFreshness`] with
    /// `mqtt input not fresh` for a read without a new message and with the link condition while
    /// the session is down, and [`RuntimeError::IoDriver`] with the text of a payload that does not
    /// convert. Under `warn` and `ignore` the same conditions degrade the health, the last
    /// accepted snapshot is copied, and the read succeeds.
    pub fn read_inputs(&mut self, now_ms: u64, image: &mut [u8]) -> Result<(), RuntimeError> {
        if !self.params.input_enabled() {
            return Ok(());
        }
        let Link::Connected(session) = &self.link else {
            let text = self.link_text(now_ms);
            let condition = self.policy_health(&text);
            if self.params.output_enabled() {
                self.paths.link = condition.clone();
            }
            self.paths.input = condition;
            return self.held_inputs(RuntimeError::IoFreshness(SmolStr::from(text)), image);
        };
        let refused = session.refused.clone();
        if self.params.output_enabled() {
            self.paths.link = IoDriverHealth::Ok;
        }
        let batch: Vec<Message> = self.queue.drain(..).collect();
        self.queued_bytes = 0;
        if batch.is_empty() {
            let text = match refused {
                Some(topic) => format!(
                    "{NOT_FRESH}; the broker refused the subscription to {}",
                    quoted(&topic)
                ),
                None => String::from(NOT_FRESH),
            };
            self.paths.input = self.policy_health(&text);
            return self.held_inputs(RuntimeError::IoFreshness(SmolStr::from(text)), image);
        }
        match self.accept(batch, image.len()) {
            Ok(()) => {
                self.paths.input = IoDriverHealth::Ok;
                self.copy_snapshot(image);
                Ok(())
            }
            Err(text) => {
                self.paths.input = self.policy_health(&text);
                self.held_inputs(RuntimeError::IoDriver(SmolStr::from(text)), image)
            }
        }
    }

    /// Hands `image`, the output image, off at `now_ms`, without waiting (specification 32 §4 and
    /// §9; specification 11): in raw mode to `topic_out`, with typed points each point to its
    /// topic, and with Sparkplug one NDATA of every point. The handoff is level and latest-value:
    /// while the session is down, or while an earlier handoff waits for the transport, the image
    /// waits in place of any older one and is handed off when it can be. A driver without outputs
    /// publishes nothing.
    ///
    /// # Errors
    ///
    /// Under `on_error = "fault"`, [`RuntimeError::IoTransport`] with the output path's condition:
    /// the link condition while the session is down, `mqtt output handoff pending` while an earlier
    /// handoff waits, or the text of a point that does not convert, in which case nothing is
    /// published. Under `warn` and `ignore` the same conditions degrade the health and the write
    /// succeeds.
    pub fn write_outputs(&mut self, now_ms: u64, image: &[u8]) -> Result<(), RuntimeError> {
        if !self.params.output_enabled() {
            return Ok(());
        }
        if !self.is_connected() {
            self.pending = Some(image.to_vec());
            let text = self.link_text(now_ms);
            self.paths.link = self.policy_health(&text);
            return self.output_result();
        }
        self.paths.link = IoDriverHealth::Ok;
        if self.handoff_end.is_some() || self.pending.is_some() {
            self.pending = Some(image.to_vec());
            self.paths.publish = self.policy_health(HANDOFF_PENDING);
            return self.output_result();
        }
        self.publish_image(now_ms, image);
        self.output_result()
    }

    /// Whether a write at `now_ms` would wait for the transport: a handoff in flight or waiting,
    /// or a stream that opens or is about to. A runtime whose transport runs on another thread
    /// gives a write up to [`SCAN_WAIT_MS`](super::SCAN_WAIT_MS) while this holds.
    #[must_use]
    pub fn output_settling(&self, now_ms: u64) -> bool {
        if !self.params.output_enabled() {
            return false;
        }
        match &self.link {
            Link::Connected(_) => self.handoff_end.is_some() || self.pending.is_some(),
            Link::Opening { .. } | Link::AwaitingConnAck { .. } => true,
            Link::Down { until_ms, .. } => now_ms >= *until_ms,
            Link::Stopping | Link::Stopped => false,
        }
    }

    /// The driver's health: the more severe of the input and output paths, the output path's
    /// detail on a tie, and degraded with `incoming queue overflow: <n> messages dropped` from the
    /// first dropped message on, unless it is faulted (specification 34 §11.1).
    #[must_use]
    pub fn health(&self) -> IoDriverHealth {
        let health = combine(&self.paths.input, &self.output_health());
        if self.dropped == 0 || matches!(health, IoDriverHealth::Faulted { .. }) {
            return health;
        }
        IoDriverHealth::Degraded {
            error: SmolStr::new(format!(
                "incoming queue overflow: {} messages dropped",
                self.dropped
            )),
        }
    }

    /// The output path's health, which confirms a safe-state handoff (specification 11): the more
    /// severe of the link and the publish condition, the publish condition's detail on a tie;
    /// always `Ok` for a driver without outputs.
    #[must_use]
    pub fn output_health(&self) -> IoDriverHealth {
        if !self.params.output_enabled() {
            return IoDriverHealth::Ok;
        }
        combine(&self.paths.link, &self.paths.publish)
    }

    /// Hands `image` off at `now_ms` to a running session with no handoff in flight: its packets
    /// join the outgoing bytes; when they do not fit the outgoing bound yet, the image waits; and
    /// when a point does not convert or the packets are larger than the bound, nothing is published
    /// and the publish condition says why.
    pub(super) fn publish_image(&mut self, now_ms: u64, image: &[u8]) {
        let seq = self.sparkplug_seq;
        match self.handoff_bytes(now_ms, image) {
            Ok(bytes) if bytes.len() > self.limits.outgoing_bytes => {
                self.sparkplug_seq = seq;
                let text = self.no_room("the handoff", bytes.len());
                self.paths.publish = self.policy_health(&text);
            }
            Ok(bytes) if !self.queue_bytes(now_ms, &bytes) => {
                // The transport has not taken enough of the bytes before it: the image waits, and
                // its Sparkplug sequence number with it.
                self.sparkplug_seq = seq;
                self.pending = Some(image.to_vec());
                self.paths.publish = self.policy_health(HANDOFF_PENDING);
            }
            Ok(_) => {
                self.handoff_end = Some(self.outgoing.len());
                self.paths.publish = IoDriverHealth::Ok;
                if self.params.sparkplug.is_some() {
                    // The values a rebirth's NBIRTH carries.
                    let last = self.last_image.get_or_insert_with(Vec::new);
                    last.clear();
                    last.extend_from_slice(image);
                }
            }
            Err(text) => self.paths.publish = self.policy_health(&text),
        }
    }

    /// The PUBLISH packets of `image`, all or none. The Sparkplug sequence number advances only
    /// when the NDATA is made.
    fn handoff_bytes(&mut self, now_ms: u64, image: &[u8]) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        if self.params.output_points.is_empty() {
            bytes.extend(
                publish_packet(&self.params.topic_out, image, self.limits.max_packet_bytes)
                    .map_err(publish_failure)?,
            );
        } else if let Some(sparkplug) = &self.params.sparkplug {
            let seq = self.sparkplug_seq;
            let payload = sparkplug::encode_ndata(
                &self.params.output_points,
                image,
                u64::from(seq),
                self.utc_ms(now_ms),
            )?;
            bytes.extend(
                publish_packet(
                    &sparkplug.ndata_topic(),
                    &payload,
                    self.limits.max_packet_bytes,
                )
                .map_err(publish_failure)?,
            );
            self.sparkplug_seq = seq.wrapping_add(1);
        } else {
            for point in &self.params.output_points {
                let payload = payload::encode_output(point, image)?;
                bytes.extend(
                    publish_packet(&point.topic, &payload, self.limits.max_packet_bytes)
                        .map_err(publish_failure)?,
                );
            }
        }
        Ok(bytes)
    }

    /// The write's result: an error with the output path's detail when it is faulted.
    fn output_result(&self) -> Result<(), RuntimeError> {
        match self.output_health() {
            IoDriverHealth::Faulted { error } => Err(RuntimeError::IoTransport(error)),
            IoDriverHealth::Ok | IoDriverHealth::Degraded { .. } => Ok(()),
        }
    }

    /// A read's result for a condition already set: under `fault`, `error` with the image
    /// unchanged; otherwise the last accepted snapshot copied, or, without one, the condition
    /// `mqtt input snapshot unavailable`, and success.
    fn held_inputs(&mut self, error: RuntimeError, image: &mut [u8]) -> Result<(), RuntimeError> {
        if self.params.on_error == OnError::Fault {
            return Err(error);
        }
        if !self.copy_snapshot(image) {
            self.paths.input = self.policy_health(SNAPSHOT_UNAVAILABLE);
        }
        Ok(())
    }

    /// Accepts the batch `batch` into the snapshot for an input image of `image_len` bytes: the
    /// newest payload in raw mode; each message converted to its point otherwise, none taken when
    /// one fails.
    fn accept(&mut self, mut batch: Vec<Message>, image_len: usize) -> Result<(), String> {
        match &mut self.snapshot {
            Snapshot::Raw(newest) => {
                if let Some(message) = batch.pop() {
                    *newest = Some(message.payload);
                }
                Ok(())
            }
            Snapshot::Points(values) => {
                let mut candidate = values.clone();
                for message in &batch {
                    let Some(point) = self.params.input_points.get(message.topic) else {
                        continue;
                    };
                    let value = payload::decode_input(point, &message.payload)?;
                    payload::check_input_fits(point, image_len)?;
                    if let Some(slot) = candidate.get_mut(message.topic) {
                        *slot = Some(value);
                    }
                }
                *values = candidate;
                Ok(())
            }
        }
    }

    /// Copies the last accepted snapshot into `image`; false when there is none, or when a point
    /// does not fit the image, which then stays as it was.
    fn copy_snapshot(&self, image: &mut [u8]) -> bool {
        match &self.snapshot {
            Snapshot::Raw(None) => false,
            Snapshot::Raw(Some(payload)) => {
                let bytes = payload.iter().copied().chain(core::iter::repeat(0));
                for (slot, byte) in image.iter_mut().zip(bytes) {
                    *slot = byte;
                }
                true
            }
            Snapshot::Points(values) => {
                let held: Vec<(&InputPoint, ImageValue)> = self
                    .params
                    .input_points
                    .iter()
                    .zip(values)
                    .filter_map(|(point, value)| value.map(|value| (point, value)))
                    .collect();
                let len = image.len();
                if held.is_empty()
                    || held
                        .iter()
                        .any(|(point, _)| payload::check_input_fits(point, len).is_err())
                {
                    return false;
                }
                held.into_iter()
                    .all(|(point, value)| payload::write_input(image, point, value).is_ok())
            }
        }
    }
}

/// The text of a publication that cannot be made, `text` after `mqtt publish: ` as
/// trust-runtime's driver writes it.
fn publish_failure(mut text: String) -> String {
    text.insert_str(0, "mqtt publish: ");
    text
}

/// The more severe of two conditions; `tie_winner` on a tie.
fn combine(other: &IoDriverHealth, tie_winner: &IoDriverHealth) -> IoDriverHealth {
    if rank(other) > rank(tie_winner) {
        other.clone()
    } else {
        tie_winner.clone()
    }
}

/// The severity of a condition.
fn rank(health: &IoDriverHealth) -> u8 {
    match health {
        IoDriverHealth::Ok => 0,
        IoDriverHealth::Degraded { .. } => 1,
        IoDriverHealth::Faulted { .. } => 2,
    }
}
