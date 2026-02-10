use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::io::{IoAddress, ModbusTcpDriver, MqttIoDriver};
use trust_runtime::task::ProgramDef;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MqttPublish {
    topic: String,
    payload: Vec<u8>,
}

#[derive(Debug, Default)]
struct MqttBrokerState {
    connect_count: usize,
    subscribe_count: usize,
    publishes: Vec<MqttPublish>,
}

fn start_mqtt_test_broker(
    topic_in: &str,
    inbound_payload: Option<Vec<u8>>,
    expected_publishes: usize,
) -> (SocketAddr, Arc<Mutex<MqttBrokerState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mqtt test broker");
    let addr = listener.local_addr().expect("mqtt broker address");
    let state = Arc::new(Mutex::new(MqttBrokerState::default()));
    let state_ref = Arc::clone(&state);
    let topic_in = topic_in.to_string();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept mqtt client");
        let _ = stream.set_read_timeout(Some(StdDuration::from_millis(200)));
        let _ = stream.set_write_timeout(Some(StdDuration::from_secs(2)));

        let mut sent_inbound = false;
        let mut idle_timeouts = 0usize;
        loop {
            match read_mqtt_packet(&mut stream) {
                Ok((header, packet)) => {
                    idle_timeouts = 0;
                    match header >> 4 {
                        1 => {
                            if write_mqtt_connack(&mut stream).is_err() {
                                break;
                            }
                            let mut guard = state_ref
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner());
                            guard.connect_count += 1;
                        }
                        3 => {
                            if let Some(publish) = parse_publish(header, &packet) {
                                let mut guard = state_ref
                                    .lock()
                                    .unwrap_or_else(|poison| poison.into_inner());
                                guard.publishes.push(publish);
                                if expected_publishes > 0
                                    && guard.publishes.len() >= expected_publishes
                                {
                                    break;
                                }
                            }
                        }
                        8 => {
                            let Some((packet_id, topic_count)) = parse_subscribe(&packet) else {
                                break;
                            };
                            if write_mqtt_suback(&mut stream, packet_id, topic_count).is_err() {
                                break;
                            }
                            let mut guard = state_ref
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner());
                            guard.subscribe_count += topic_count;
                            drop(guard);

                            if !sent_inbound {
                                if let Some(payload) = inbound_payload.as_ref() {
                                    if write_mqtt_publish(&mut stream, &topic_in, payload).is_err()
                                    {
                                        break;
                                    }
                                }
                                sent_inbound = true;
                            }
                        }
                        12 => {
                            if write_mqtt_pingresp(&mut stream).is_err() {
                                break;
                            }
                        }
                        14 => break,
                        _ => {}
                    }
                }
                Err(err) if matches!(err.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                    idle_timeouts += 1;
                    if idle_timeouts > 20 {
                        break;
                    }
                }
                Err(err) if err.kind() == ErrorKind::UnexpectedEof => break,
                Err(_) => break,
            }
        }
    });

    (addr, state)
}

fn read_mqtt_packet(stream: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    let mut header = [0u8; 1];
    stream.read_exact(&mut header)?;
    let len = read_remaining_length(stream)?;
    let mut packet = vec![0u8; len];
    stream.read_exact(&mut packet)?;
    Ok((header[0], packet))
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

fn write_mqtt_connack(stream: &mut TcpStream) -> io::Result<()> {
    write_mqtt_packet(stream, 0x20, &[0x00, 0x00])
}

fn write_mqtt_pingresp(stream: &mut TcpStream) -> io::Result<()> {
    write_mqtt_packet(stream, 0xD0, &[])
}

fn parse_subscribe(packet: &[u8]) -> Option<(u16, usize)> {
    if packet.len() < 2 {
        return None;
    }
    let packet_id = u16::from_be_bytes([packet[0], packet[1]]);
    let mut idx = 2usize;
    let mut count = 0usize;
    while idx + 2 <= packet.len() {
        let topic_len = u16::from_be_bytes([packet[idx], packet[idx + 1]]) as usize;
        idx += 2;
        if idx + topic_len + 1 > packet.len() {
            return None;
        }
        idx += topic_len;
        idx += 1;
        count += 1;
    }
    if idx != packet.len() || count == 0 {
        return None;
    }
    Some((packet_id, count))
}

fn write_mqtt_suback(stream: &mut TcpStream, packet_id: u16, topic_count: usize) -> io::Result<()> {
    let mut payload = Vec::with_capacity(2 + topic_count);
    payload.extend_from_slice(&packet_id.to_be_bytes());
    payload.extend(std::iter::repeat_n(0u8, topic_count));
    write_mqtt_packet(stream, 0x90, &payload)
}

fn parse_publish(header: u8, packet: &[u8]) -> Option<MqttPublish> {
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

fn write_mqtt_publish(stream: &mut TcpStream, topic: &str, payload: &[u8]) -> io::Result<()> {
    let topic_bytes = topic.as_bytes();
    let mut packet = Vec::with_capacity(2 + topic_bytes.len() + payload.len());
    packet.extend_from_slice(&(topic_bytes.len() as u16).to_be_bytes());
    packet.extend_from_slice(topic_bytes);
    packet.extend_from_slice(payload);
    write_mqtt_packet(stream, 0x30, &packet)
}

fn start_modbus_server(regs: Arc<Mutex<Vec<u16>>>, requests: usize) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind modbus test server");
    let addr = listener.local_addr().expect("modbus server addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept modbus client");
        let _ = stream.set_read_timeout(Some(StdDuration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(StdDuration::from_secs(2)));
        for _ in 0..requests {
            if handle_modbus_request(&mut stream, &regs).is_err() {
                break;
            }
        }
    });
    addr
}

fn handle_modbus_request(stream: &mut TcpStream, regs: &Arc<Mutex<Vec<u16>>>) -> Result<(), ()> {
    let mut header = [0u8; 6];
    stream.read_exact(&mut header).map_err(|_| ())?;
    let tx = u16::from_be_bytes([header[0], header[1]]);
    let length = u16::from_be_bytes([header[4], header[5]]) as usize;
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).map_err(|_| ())?;
    if body.len() < 2 {
        return Err(());
    }
    let unit_id = body[0];
    let pdu = &body[1..];
    let function = pdu[0];
    let response = match function {
        0x04 => handle_read_input(pdu, regs),
        0x10 => handle_write_multiple(pdu, regs),
        _ => vec![function | 0x80, 0x01],
    };
    let mut resp_header = [0u8; 6];
    resp_header[0..2].copy_from_slice(&tx.to_be_bytes());
    resp_header[2..4].copy_from_slice(&0u16.to_be_bytes());
    resp_header[4..6].copy_from_slice(&((response.len() + 1) as u16).to_be_bytes());
    stream.write_all(&resp_header).map_err(|_| ())?;
    stream.write_all(&[unit_id]).map_err(|_| ())?;
    stream.write_all(&response).map_err(|_| ())?;
    stream.flush().ok();
    Ok(())
}

fn handle_read_input(pdu: &[u8], regs: &Arc<Mutex<Vec<u16>>>) -> Vec<u8> {
    if pdu.len() < 5 {
        return vec![0x84, 0x03];
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
    let guard = regs.lock().expect("regs lock");
    if start + qty > guard.len() {
        return vec![0x84, 0x02];
    }
    let mut payload = Vec::with_capacity(2 + qty * 2);
    payload.push(0x04);
    payload.push((qty * 2) as u8);
    for reg in &guard[start..start + qty] {
        payload.push((reg >> 8) as u8);
        payload.push(*reg as u8);
    }
    payload
}

fn handle_write_multiple(pdu: &[u8], regs: &Arc<Mutex<Vec<u16>>>) -> Vec<u8> {
    if pdu.len() < 6 {
        return vec![0x90, 0x03];
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
    let byte_count = pdu[5] as usize;
    if pdu.len() < 6 + byte_count {
        return vec![0x90, 0x03];
    }
    let mut guard = regs.lock().expect("regs lock");
    if start + qty > guard.len() {
        return vec![0x90, 0x02];
    }
    for idx in 0..qty {
        let offset = 6 + idx * 2;
        let hi = pdu.get(offset).copied().unwrap_or(0);
        let lo = pdu.get(offset + 1).copied().unwrap_or(0);
        guard[start + idx] = u16::from_be_bytes([hi, lo]);
    }
    vec![
        0x10,
        (start >> 8) as u8,
        start as u8,
        (qty >> 8) as u8,
        qty as u8,
    ]
}

#[test]
fn runtime_composes_modbus_and_mqtt_drivers_live() {
    let regs = Arc::new(Mutex::new(vec![0u16; 4]));
    {
        let mut guard = regs.lock().expect("regs lock");
        guard[0] = 0x0100;
    }
    let modbus_addr = start_modbus_server(regs.clone(), 512);

    let topic_in = "trust/test/in";
    let topic_out = "trust/test/out";
    let (mqtt_addr, mqtt_state) = start_mqtt_test_broker(topic_in, None, 1);

    let mut runtime = Runtime::new();
    runtime.io_mut().resize(1, 1, 0);
    runtime.storage_mut().set_global("in", Value::Bool(false));
    runtime.storage_mut().set_global("out", Value::Bool(false));
    let program = ProgramDef {
        name: "P".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("out".into()),
            value: Expr::Name("in".into()),
            location: None,
        }],
    };
    runtime.register_program(program).expect("register program");
    runtime.io_mut().bind(
        "in",
        IoAddress::parse("%IX0.0").expect("parse input address"),
    );
    runtime.io_mut().bind(
        "out",
        IoAddress::parse("%QX0.0").expect("parse output address"),
    );

    let modbus_params: toml::Value = toml::from_str(&format!(
        "address = \"{modbus_addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\n"
    ))
    .expect("parse modbus params");
    runtime.add_io_driver(
        "modbus-tcp",
        Box::new(ModbusTcpDriver::from_params(&modbus_params).expect("create modbus driver")),
    );

    let mqtt_params: toml::Value = toml::from_str(&format!(
        "broker = \"{mqtt_addr}\"\ntopic_in = \"{topic_in}\"\ntopic_out = \"{topic_out}\"\nreconnect_ms = 10\n"
    ))
    .expect("parse mqtt params");
    runtime.add_io_driver(
        "mqtt",
        Box::new(MqttIoDriver::from_params(&mqtt_params).expect("create mqtt driver")),
    );

    let deadline = Instant::now() + StdDuration::from_secs(3);
    let mut outbound_payload = None;
    while Instant::now() < deadline {
        runtime.execute_cycle().expect("execute cycle");
        if let Some(publish) = {
            let guard = mqtt_state.lock().expect("mqtt state lock");
            guard
                .publishes
                .iter()
                .find(|entry| entry.topic == topic_out)
                .cloned()
        } {
            outbound_payload = Some(publish.payload);
            break;
        }
        thread::sleep(StdDuration::from_millis(20));
    }

    let outbound_payload = outbound_payload.expect("mqtt publish should be observed");
    assert_eq!(
        outbound_payload.first().copied().unwrap_or(0) & 0x01,
        0x01,
        "mqtt published payload should reflect output image bit"
    );

    let mqtt = mqtt_state.lock().expect("mqtt state lock");
    assert!(
        mqtt.connect_count >= 1,
        "expected mqtt CONNECT handshake to be observed"
    );
    assert!(
        mqtt.subscribe_count >= 1,
        "expected mqtt SUBSCRIBE to topic_in to be observed"
    );
    drop(mqtt);

    let guard = regs.lock().expect("regs lock");
    assert_eq!(
        guard[0], 0x0100,
        "modbus register write should reflect output image bit"
    );
}
