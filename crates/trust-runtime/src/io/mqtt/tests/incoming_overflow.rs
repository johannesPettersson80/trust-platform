// Incoming-queue overflow policy and the driver health it reports (spec 34 section 11.1).
// Included into `io::mqtt::tests`, which supplies `packet`, `params` and the imports.

struct OverflowSession {
    incoming: Arc<Mutex<MqttIncomingQueue>>,
}

impl MqttSession for OverflowSession {
    fn is_connected(&self) -> bool {
        true
    }

    fn take_payloads(&mut self) -> Vec<MqttInboundPayload> {
        let mut guard = self.incoming.lock().unwrap_or_else(|e| e.into_inner());
        guard.drain()
    }

    fn publish(&mut self, _topic: &str, _payload: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn last_error(&self) -> Option<SmolStr> {
        None
    }
}

/// Hands out sessions that read one shared queue, the way the event loop fills it.
struct OverflowFactory {
    incoming: Arc<Mutex<MqttIncomingQueue>>,
    dropped: Arc<Mutex<u64>>,
}

impl MqttSessionFactory for OverflowFactory {
    fn connect(&self, _config: &MqttIoConfig) -> Result<Box<dyn MqttSession>, RuntimeError> {
        Ok(Box::new(OverflowSession {
            incoming: Arc::clone(&self.incoming),
        }))
    }

    fn incoming_dropped(&self) -> u64 {
        *self.dropped.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Payload `index` as two big-endian bytes, so a driver input shows which message it holds.
fn overflow_packet(index: usize) -> MqttInboundPayload {
    packet("line/in", (index as u16).to_be_bytes().to_vec())
}

fn queue_with_pushes(pushes: usize) -> (Arc<Mutex<u64>>, MqttIncomingQueue) {
    let dropped = Arc::new(Mutex::new(0));
    let mut queue = MqttIncomingQueue::new(Arc::clone(&dropped));
    for index in 0..pushes {
        queue.push(overflow_packet(index));
    }
    (dropped, queue)
}

fn dropped_count(dropped: &Arc<Mutex<u64>>) -> u64 {
    *dropped.lock().unwrap_or_else(|e| e.into_inner())
}

fn overflow_driver(
    incoming: Arc<Mutex<MqttIncomingQueue>>,
    dropped: Arc<Mutex<u64>>,
) -> MqttIoDriver {
    MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
topic_in = "line/in"
topic_out = "line/out"
"#,
        ),
        Arc::new(OverflowFactory { incoming, dropped }),
    )
    .expect("construct mqtt driver")
}

#[test]
fn incoming_queue_at_bound_drops_nothing() {
    let (dropped, mut queue) = queue_with_pushes(256);
    assert_eq!(dropped_count(&dropped), 0);
    let kept = queue.drain();
    assert_eq!(kept.len(), 256);
    assert_eq!(kept.first(), Some(&overflow_packet(0)));
    assert_eq!(kept.last(), Some(&overflow_packet(255)));
}

#[test]
fn incoming_queue_257th_message_keeps_newest_256_and_counts_one_drop() {
    let (dropped, mut queue) = queue_with_pushes(257);
    assert_eq!(dropped_count(&dropped), 1);
    let kept = queue.drain();
    assert_eq!(kept.len(), 256);
    assert_eq!(kept.first(), Some(&overflow_packet(1)));
    assert_eq!(kept.last(), Some(&overflow_packet(256)));
}

#[test]
fn incoming_queue_counts_every_dropped_message() {
    let (dropped, mut queue) = queue_with_pushes(300);
    assert_eq!(dropped_count(&dropped), 44);
    let kept = queue.drain();
    assert_eq!(kept.len(), 256);
    assert_eq!(kept.first(), Some(&overflow_packet(44)));
    assert_eq!(kept.last(), Some(&overflow_packet(299)));
}

#[test]
fn driver_with_256_queued_messages_stays_healthy() {
    let (dropped, queue) = queue_with_pushes(256);
    let mut driver = overflow_driver(Arc::new(Mutex::new(queue)), dropped);
    let mut inputs = [0u8; 2];
    driver.read_inputs(&mut inputs).expect("read inputs");
    assert_eq!(inputs, [0x00, 0xff]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

#[test]
fn driver_after_257th_message_reports_exact_detail_and_keeps_inputs_fresh() {
    let (dropped, queue) = queue_with_pushes(257);
    let incoming = Arc::new(Mutex::new(queue));
    let mut driver = overflow_driver(Arc::clone(&incoming), Arc::clone(&dropped));
    let degraded = IoDriverHealth::Degraded {
        error: SmolStr::new("incoming queue overflow: 1 messages dropped"),
    };

    let mut inputs = [0u8; 2];
    driver.read_inputs(&mut inputs).expect("read inputs");
    assert_eq!(inputs, [0x01, 0x00]);
    assert_eq!(driver.health(), degraded);

    // Degraded by the overflow alone, the next message still reaches the inputs.
    incoming
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(overflow_packet(257));
    driver
        .read_inputs(&mut inputs)
        .expect("read after overflow");
    assert_eq!(inputs, [0x01, 0x01]);
    assert_eq!(driver.health(), degraded);
}

#[test]
fn overflowed_driver_reports_degraded_health_with_ok_output_path() {
    let (dropped, queue) = queue_with_pushes(257);
    let mut driver = overflow_driver(Arc::new(Mutex::new(queue)), dropped);
    let mut inputs = [0u8; 2];
    driver.read_inputs(&mut inputs).expect("read inputs");
    let degraded = IoDriverHealth::Degraded {
        error: SmolStr::new("incoming queue overflow: 1 messages dropped"),
    };
    assert_eq!(driver.health(), degraded);

    // A safe-state handoff is confirmed by a successful write and the output path, which the
    // overflow does not touch (io_subsystem write_safe_outputs).
    driver.write_outputs(&[0x5a]).expect("write outputs");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(driver.health(), degraded);
}

#[test]
fn incoming_overflow_never_overrides_a_faulted_driver() {
    let faulted = IoDriverHealth::Faulted {
        error: SmolStr::new("mqtt fault"),
    };
    assert_eq!(overlay_incoming_overflow(faulted.clone(), 3), faulted);
    assert_eq!(
        overlay_incoming_overflow(IoDriverHealth::Ok, 0),
        IoDriverHealth::Ok
    );
}
