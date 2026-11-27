//! MQTT 3.1.1 for the tests that run a driver or a project against a broker (OASIS MQTT Version
//! 3.1.1, 29 October 2014): the packets of a client's session, read and written here from the
//! standard, and [`MqttTestBroker`], a broker stand-in on the loopback. The broker answers CONNECT
//! with CONNACK (§3.2), SUBSCRIBE with SUBACK (§3.9) and PINGREQ with PINGRESP (§3.13), records the
//! messages each client publishes (§3.3), and delivers the messages a test gives it to the clients
//! that subscribed to their topic, at quality of service 0. A test that runs its own broker loop
//! uses the packet functions alone. Included with `#[path = "support/mqtt.rs"] mod mqtt;`.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

/// A PUBLISH as the broker read it (§3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttPublish {
    pub topic: String,
    pub payload: Vec<u8>,
}

/// Reads one packet: its fixed header's first byte and the rest of the packet after the remaining
/// length (§2.2). The first byte waits for the stream's read timeout; the rest gets two seconds.
pub fn read_mqtt_packet(stream: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    let mut header = [0u8; 1];
    stream.read_exact(&mut header)?;
    let idle_timeout = stream.read_timeout().ok().flatten();
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let result = read_mqtt_packet_after_header(stream, header[0]);
    let _ = stream.set_read_timeout(idle_timeout);
    result
}

fn read_mqtt_packet_after_header(stream: &mut TcpStream, header: u8) -> io::Result<(u8, Vec<u8>)> {
    let len = read_remaining_length(stream)?;
    let mut packet = vec![0u8; len];
    stream.read_exact(&mut packet)?;
    Ok((header, packet))
}

fn read_remaining_length(stream: &mut TcpStream) -> io::Result<usize> {
    let mut multiplier = 1usize;
    let mut value = 0usize;
    for _ in 0..4 {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte)?;
        value += ((byte[0] & 0x7f) as usize) * multiplier;
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
        multiplier *= 128;
    }
    Err(io::Error::new(
        ErrorKind::InvalidData,
        "invalid mqtt remaining length",
    ))
}

fn encode_remaining_length(mut len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4);
    loop {
        let mut byte = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if len == 0 {
            break;
        }
    }
    bytes
}

fn write_mqtt_packet(stream: &mut TcpStream, header: u8, payload: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(1 + 4 + payload.len());
    frame.push(header);
    frame.extend(encode_remaining_length(payload.len()));
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()
}

/// CONNACK, session not present, connection accepted (§3.2).
pub fn write_mqtt_connack(stream: &mut TcpStream) -> io::Result<()> {
    write_mqtt_packet(stream, 0x20, &[0x00, 0x00])
}

/// PINGRESP (§3.13).
pub fn write_mqtt_pingresp(stream: &mut TcpStream) -> io::Result<()> {
    write_mqtt_packet(stream, 0xD0, &[])
}

/// The packet identifier and the topic filters of a SUBSCRIBE (§3.8), or None when it is
/// malformed or names no topic.
pub fn parse_subscribe(packet: &[u8]) -> Option<(u16, Vec<String>)> {
    if packet.len() < 2 {
        return None;
    }
    let packet_id = u16::from_be_bytes([packet[0], packet[1]]);
    let mut idx = 2usize;
    let mut topics = Vec::new();
    while idx + 2 <= packet.len() {
        let topic_len = u16::from_be_bytes([packet[idx], packet[idx + 1]]) as usize;
        idx += 2;
        if idx + topic_len + 1 > packet.len() {
            return None;
        }
        topics.push(String::from_utf8(packet[idx..idx + topic_len].to_vec()).ok()?);
        idx += topic_len;
        idx += 1;
    }
    if idx != packet.len() || topics.is_empty() {
        return None;
    }
    Some((packet_id, topics))
}

/// SUBACK granting quality of service 0 to each of `topic_count` topics (§3.9).
pub fn write_mqtt_suback(
    stream: &mut TcpStream,
    packet_id: u16,
    topic_count: usize,
) -> io::Result<()> {
    let mut payload = Vec::with_capacity(2 + topic_count);
    payload.extend_from_slice(&packet_id.to_be_bytes());
    payload.extend(std::iter::repeat_n(0u8, topic_count));
    write_mqtt_packet(stream, 0x90, &payload)
}

/// The topic and payload of a PUBLISH (§3.3), or None when it is malformed.
pub fn parse_publish(header: u8, packet: &[u8]) -> Option<MqttPublish> {
    if packet.len() < 2 {
        return None;
    }
    let topic_len = u16::from_be_bytes([packet[0], packet[1]]) as usize;
    if packet.len() < 2 + topic_len {
        return None;
    }
    let topic = std::str::from_utf8(&packet[2..2 + topic_len])
        .ok()?
        .to_string();
    let qos = (header >> 1) & 0x03;
    let mut idx = 2 + topic_len;
    if qos > 0 {
        if packet.len() < idx + 2 {
            return None;
        }
        idx += 2;
    }
    Some(MqttPublish {
        topic,
        payload: packet[idx..].to_vec(),
    })
}

/// PUBLISH of `payload` to `topic` at quality of service 0 (§3.3).
pub fn write_mqtt_publish(stream: &mut TcpStream, topic: &str, payload: &[u8]) -> io::Result<()> {
    let topic_bytes = topic.as_bytes();
    let mut packet = Vec::with_capacity(2 + topic_bytes.len() + payload.len());
    packet.extend_from_slice(&(topic_bytes.len() as u16).to_be_bytes());
    packet.extend_from_slice(topic_bytes);
    packet.extend_from_slice(payload);
    write_mqtt_packet(stream, 0x30, &packet)
}

#[derive(Debug, Default)]
struct Record {
    published: Vec<MqttPublish>,
    subscriptions: Vec<String>,
    /// Every message a test gave to deliver, in order; each session sends those after its
    /// subscription whose topic it subscribed to, as a broker forwards a message that is not
    /// retained only to the subscribers it has when the message comes.
    deliveries: Vec<MqttPublish>,
}

/// A broker stand-in on a free loopback port, stopped when it is dropped.
pub struct MqttTestBroker {
    address: SocketAddr,
    record: Arc<Mutex<Record>>,
    stop: Arc<AtomicBool>,
}

impl MqttTestBroker {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the mqtt test broker");
        listener
            .set_nonblocking(true)
            .expect("a nonblocking listener");
        let address = listener.local_addr().expect("the broker's address");
        let record = Arc::new(Mutex::new(Record::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (accepted_record, accepted_stop) = (Arc::clone(&record), Arc::clone(&stop));
        thread::spawn(move || {
            while !accepted_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (record, stop) =
                            (Arc::clone(&accepted_record), Arc::clone(&accepted_stop));
                        thread::spawn(move || serve(stream, &record, &stop));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            address,
            record,
            stop,
        }
    }

    /// `127.0.0.1:<port>`, for a driver's `broker` parameter.
    pub fn endpoint(&self) -> String {
        self.address.to_string()
    }

    /// The topic filters the clients subscribed to, in order.
    pub fn subscriptions(&self) -> Vec<String> {
        lock(&self.record).subscriptions.clone()
    }

    /// The payloads the clients published to `topic`, in order.
    pub fn published(&self, topic: &str) -> Vec<Vec<u8>> {
        lock(&self.record)
            .published
            .iter()
            .filter(|publish| publish.topic == topic)
            .map(|publish| publish.payload.clone())
            .collect()
    }

    /// Delivers `payload` on `topic` to each session that has subscribed to `topic`.
    pub fn deliver(&self, topic: &str, payload: &[u8]) {
        lock(&self.record).deliveries.push(MqttPublish {
            topic: topic.to_string(),
            payload: payload.to_vec(),
        });
    }
}

impl Drop for MqttTestBroker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn lock(record: &Mutex<Record>) -> MutexGuard<'_, Record> {
    record.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One client's session: answers its packets and sends it the deliveries due, until it
/// disconnects, the connection fails or the broker stops.
fn serve(mut stream: TcpStream, record: &Mutex<Record>, stop: &AtomicBool) {
    // Accepted from a nonblocking listener: Windows hands the flag on to the stream.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut topics: Vec<String> = Vec::new();
    let mut sent = 0usize;
    while !stop.load(Ordering::Relaxed) {
        match read_mqtt_packet(&mut stream) {
            Ok((header, packet)) => match header >> 4 {
                1 if write_mqtt_connack(&mut stream).is_err() => return,
                3 => {
                    if let Some(publish) = parse_publish(header, &packet) {
                        lock(record).published.push(publish);
                    }
                }
                8 => {
                    let Some((packet_id, subscribed)) = parse_subscribe(&packet) else {
                        return;
                    };
                    if write_mqtt_suback(&mut stream, packet_id, subscribed.len()).is_err() {
                        return;
                    }
                    let mut guard = lock(record);
                    sent = guard.deliveries.len();
                    guard.subscriptions.extend(subscribed.iter().cloned());
                    topics.extend(subscribed);
                }
                12 if write_mqtt_pingresp(&mut stream).is_err() => return,
                14 => return,
                _ => {}
            },
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {}
            Err(_) => return,
        }
        let due: Vec<MqttPublish> = {
            let guard = lock(record);
            let due = guard.deliveries[sent..]
                .iter()
                .filter(|delivery| topics.contains(&delivery.topic))
                .cloned()
                .collect();
            sent = guard.deliveries.len();
            due
        };
        for delivery in due {
            if write_mqtt_publish(&mut stream, &delivery.topic, &delivery.payload).is_err() {
                return;
            }
        }
    }
}
