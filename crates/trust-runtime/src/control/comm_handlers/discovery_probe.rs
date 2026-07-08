use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub(super) const CONFIRMED: &str = "confirmed";
pub(super) const LIKELY: &str = "likely";
pub(super) const PORT_REACHABLE: &str = "port_reachable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscoveryProbeOutcome {
    pub source: &'static str,
    pub confidence: &'static str,
    pub detail: String,
    pub warnings: Vec<String>,
    pub auth_required: bool,
}

impl DiscoveryProbeOutcome {
    fn confirmed(source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            source,
            confidence: CONFIRMED,
            detail: detail.into(),
            warnings: Vec::new(),
            auth_required: false,
        }
    }

    fn likely(source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            source,
            confidence: LIKELY,
            detail: detail.into(),
            warnings: Vec::new(),
            auth_required: false,
        }
    }

    pub(super) fn port_reachable(source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            source,
            confidence: PORT_REACHABLE,
            detail: detail.into(),
            warnings: Vec::new(),
            auth_required: false,
        }
    }

    fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    fn with_auth_required(mut self) -> Self {
        self.auth_required = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ModbusDiscoveryProbe {
    pub unit_id: u8,
    pub safe_read: Option<ModbusSafeReadProbe>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ModbusSafeReadProbe {
    pub address: u16,
    pub quantity: u16,
}

pub(super) fn probe_modbus_tcp(
    target: SocketAddr,
    timeout: Duration,
    probe: ModbusDiscoveryProbe,
) -> Option<DiscoveryProbeOutcome> {
    let mut stream = connect(target, timeout)?;
    match send_modbus_request(&mut stream, probe.unit_id, &[0x2b, 0x0e, 0x01, 0x00], 1) {
        Ok(ModbusProbeResponse::Normal) => {
            return Some(DiscoveryProbeOutcome::confirmed(
                "modbus_device_id",
                "Modbus FC43/14 device identification responded.",
            ));
        }
        Ok(ModbusProbeResponse::Exception(code)) => {
            return Some(DiscoveryProbeOutcome::confirmed(
                "modbus_device_id_exception",
                format!("Modbus FC43/14 returned exception code {code}."),
            ));
        }
        Err(_) => {}
    }

    if let Some(safe_read) = probe.safe_read {
        let mut stream = match connect(target, timeout) {
            Some(stream) => stream,
            None => {
                return Some(DiscoveryProbeOutcome::port_reachable(
                    "tcp_connect",
                    "TCP connection succeeded, but the safe Modbus read probe could not reconnect.",
                ));
            }
        };
        let pdu = [
            0x03,
            (safe_read.address >> 8) as u8,
            safe_read.address as u8,
            (safe_read.quantity >> 8) as u8,
            safe_read.quantity as u8,
        ];
        return Some(
            match send_modbus_request(&mut stream, probe.unit_id, &pdu, 2) {
                Ok(ModbusProbeResponse::Normal) => DiscoveryProbeOutcome::confirmed(
                    "modbus_safe_read",
                    "Configured Modbus FC03 safe read responded.",
                ),
                Ok(ModbusProbeResponse::Exception(code)) => DiscoveryProbeOutcome::confirmed(
                    "modbus_safe_read_exception",
                    format!("Configured Modbus FC03 safe read returned exception code {code}."),
                ),
                Err(_) => DiscoveryProbeOutcome::port_reachable(
                    "tcp_connect",
                    "TCP connection succeeded, but Modbus protocol probes did not complete.",
                )
                .with_warning(
                    "Only TCP reachability was observed; no Modbus response was received.",
                ),
            },
        );
    }

    Some(
        DiscoveryProbeOutcome::port_reachable(
            "tcp_connect",
            "TCP connection succeeded, but Modbus FC43/14 did not respond.",
        )
        .with_warning(
            "Only TCP reachability was observed; configure a safe read probe to prove Modbus on devices without FC43/14.",
        ),
    )
}

enum ModbusProbeResponse {
    Normal,
    Exception(u8),
}

fn send_modbus_request(
    stream: &mut TcpStream,
    unit_id: u8,
    pdu: &[u8],
    transaction_id: u16,
) -> Result<ModbusProbeResponse, std::io::Error> {
    let mut header = [0u8; 6];
    header[0..2].copy_from_slice(&transaction_id.to_be_bytes());
    header[2..4].copy_from_slice(&0u16.to_be_bytes());
    header[4..6].copy_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
    stream.write_all(&header)?;
    stream.write_all(&[unit_id])?;
    stream.write_all(pdu)?;
    stream.flush()?;

    let mut response_header = [0u8; 6];
    stream.read_exact(&mut response_header)?;
    if u16::from_be_bytes([response_header[0], response_header[1]]) != transaction_id
        || u16::from_be_bytes([response_header[2], response_header[3]]) != 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid Modbus MBAP header",
        ));
    }
    let response_len = u16::from_be_bytes([response_header[4], response_header[5]]) as usize;
    if !(2..=260).contains(&response_len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid Modbus response length",
        ));
    }
    let mut response = vec![0u8; response_len];
    stream.read_exact(&mut response)?;
    let response_pdu = response.get(1..).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "missing Modbus PDU")
    })?;
    let Some(function) = response_pdu.first().copied() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "empty Modbus PDU",
        ));
    };
    if function == pdu[0] {
        return Ok(ModbusProbeResponse::Normal);
    }
    if function == (pdu[0] | 0x80) {
        return Ok(ModbusProbeResponse::Exception(
            response_pdu.get(1).copied().unwrap_or(0),
        ));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "unexpected Modbus function code",
    ))
}

pub(super) fn probe_mqtt(target: SocketAddr, timeout: Duration) -> Option<DiscoveryProbeOutcome> {
    let mut stream = connect(target, timeout)?;
    if target.port() == 8883 {
        return Some(
            DiscoveryProbeOutcome::port_reachable(
                "tcp_connect",
                "MQTTS TCP port is reachable; TLS MQTT handshake is not attempted by discovery.",
            )
            .with_warning(
                "Only TCP reachability was observed for MQTTS; configure the broker and run a connection test for TLS proof.",
            ),
        );
    }

    let client_id = format!("trust-discovery-{}", std::process::id());
    if write_mqtt_connect(&mut stream, client_id.as_bytes()).is_err() {
        return Some(DiscoveryProbeOutcome::port_reachable(
            "tcp_connect",
            "TCP connection succeeded, but MQTT CONNECT could not be written.",
        ));
    }
    match read_mqtt_connack(&mut stream) {
        Ok(0) => {
            let _ = stream.write_all(&[0xe0, 0x00]);
            Some(DiscoveryProbeOutcome::confirmed(
                "mqtt_connack",
                "MQTT CONNECT was accepted with clean_session=true.",
            ))
        }
        Ok(code @ (4 | 5)) => {
            let _ = stream.write_all(&[0xe0, 0x00]);
            Some(
                DiscoveryProbeOutcome::confirmed(
                    "mqtt_connack_auth_rejected",
                    format!("MQTT CONNACK rejected anonymous discovery with code {code}."),
                )
                .with_auth_required()
                .with_warning(
                    "MQTT broker requires credentials; discovery confirmed MQTT without storing a session.",
                ),
            )
        }
        Ok(code) => {
            let _ = stream.write_all(&[0xe0, 0x00]);
            Some(DiscoveryProbeOutcome::likely(
                "mqtt_connack_rejected",
                format!("MQTT CONNACK returned code {code}."),
            ))
        }
        Err(_) => Some(
            DiscoveryProbeOutcome::port_reachable(
                "tcp_connect",
                "TCP connection succeeded, but MQTT CONNACK was not received.",
            )
            .with_warning("Only TCP reachability was observed; no MQTT CONNACK was received."),
        ),
    }
}

fn connect(target: SocketAddr, timeout: Duration) -> Option<TcpStream> {
    let stream = TcpStream::connect_timeout(&target, timeout).ok()?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let _ = stream.set_nodelay(true);
    Some(stream)
}

fn write_mqtt_connect(stream: &mut TcpStream, client_id: &[u8]) -> Result<(), std::io::Error> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0x00, 0x04, b'M', b'Q', b'T', b'T']);
    body.push(0x04);
    body.push(0x02);
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(&(client_id.len() as u16).to_be_bytes());
    body.extend_from_slice(client_id);

    let mut packet = vec![0x10];
    encode_remaining_length(body.len(), &mut packet);
    packet.extend_from_slice(&body);
    stream.write_all(&packet)?;
    stream.flush()
}

fn read_mqtt_connack(stream: &mut TcpStream) -> Result<u8, std::io::Error> {
    let mut fixed = [0u8; 1];
    stream.read_exact(&mut fixed)?;
    if fixed[0] != 0x20 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected MQTT CONNACK",
        ));
    }
    let remaining = read_remaining_length(stream)?;
    if remaining < 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "MQTT CONNACK too short",
        ));
    }
    let mut body = vec![0u8; remaining];
    stream.read_exact(&mut body)?;
    Ok(body[1])
}

fn encode_remaining_length(mut len: usize, packet: &mut Vec<u8>) {
    loop {
        let mut byte = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            byte |= 0x80;
        }
        packet.push(byte);
        if len == 0 {
            break;
        }
    }
}

fn read_remaining_length(stream: &mut TcpStream) -> Result<usize, std::io::Error> {
    let mut multiplier = 1usize;
    let mut value = 0usize;
    for _ in 0..4 {
        let mut encoded = [0u8; 1];
        stream.read_exact(&mut encoded)?;
        value += usize::from(encoded[0] & 127) * multiplier;
        if encoded[0] & 128 == 0 {
            return Ok(value);
        }
        multiplier *= 128;
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "MQTT remaining length too long",
    ))
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use super::*;

    #[test]
    fn modbus_device_id_probe_confirms_protocol_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0u8; 11];
            stream.read_exact(&mut request).expect("read request");
            assert_eq!(&request[2..4], &[0, 0]);
            assert_eq!(request[6], 1);
            assert_eq!(&request[7..11], &[0x2b, 0x0e, 0x01, 0x00]);
            let response = [
                0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x01, 0x2b, 0x0e, 0x01, 0x01, 0x00, 0x00, 0x00,
            ];
            stream.write_all(&response).expect("write response");
        });

        let outcome = probe_modbus_tcp(
            addr,
            Duration::from_millis(250),
            ModbusDiscoveryProbe {
                unit_id: 1,
                safe_read: None,
            },
        )
        .expect("outcome");

        assert_eq!(outcome.confidence, CONFIRMED);
        assert_eq!(outcome.source, "modbus_device_id");
        handle.join().expect("join");
    }

    #[test]
    fn modbus_safe_read_probe_confirms_when_device_id_is_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let (mut first, _) = listener.accept().expect("accept first");
            let mut device_id_request = [0u8; 11];
            first
                .read_exact(&mut device_id_request)
                .expect("read device id request");
            assert_eq!(&device_id_request[7..11], &[0x2b, 0x0e, 0x01, 0x00]);
            drop(first);

            let (mut second, _) = listener.accept().expect("accept second");
            let mut read_request = [0u8; 12];
            second
                .read_exact(&mut read_request)
                .expect("read safe read request");
            assert_eq!(&read_request[7..12], &[0x03, 0x01, 0x90, 0x00, 0x01]);
            let response = [
                0x00, 0x02, 0x00, 0x00, 0x00, 0x05, 0x11, 0x03, 0x02, 0x00, 0x00,
            ];
            second.write_all(&response).expect("write response");
        });

        let outcome = probe_modbus_tcp(
            addr,
            Duration::from_millis(250),
            ModbusDiscoveryProbe {
                unit_id: 17,
                safe_read: Some(ModbusSafeReadProbe {
                    address: 400,
                    quantity: 1,
                }),
            },
        )
        .expect("outcome");

        assert_eq!(outcome.confidence, CONFIRMED);
        assert_eq!(outcome.source, "modbus_safe_read");
        handle.join().expect("join");
    }

    #[test]
    fn mqtt_probe_uses_clean_session_and_disconnects_after_connack() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("addr");
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut fixed = [0u8; 2];
            stream.read_exact(&mut fixed).expect("connect fixed header");
            assert_eq!(fixed[0], 0x10);
            let mut body = vec![0u8; fixed[1] as usize];
            stream.read_exact(&mut body).expect("connect body");
            assert_eq!(&body[0..6], &[0x00, 0x04, b'M', b'Q', b'T', b'T']);
            assert_eq!(body[6], 0x04);
            assert_eq!(body[7], 0x02, "clean_session must be set");
            stream
                .write_all(&[0x20, 0x02, 0x00, 0x00])
                .expect("connack");
            let mut disconnect = [0u8; 2];
            stream.read_exact(&mut disconnect).expect("disconnect");
            tx.send(disconnect).expect("send disconnect");
        });

        let outcome = probe_mqtt(addr, Duration::from_millis(250)).expect("outcome");

        assert_eq!(outcome.confidence, CONFIRMED);
        assert_eq!(outcome.source, "mqtt_connack");
        assert_eq!(rx.recv().expect("disconnect"), [0xe0, 0x00]);
        handle.join().expect("join");
    }

    #[test]
    fn mqtt_probe_classifies_auth_rejected_connack_separately() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut fixed = [0u8; 2];
            stream.read_exact(&mut fixed).expect("connect fixed header");
            let mut body = vec![0u8; fixed[1] as usize];
            stream.read_exact(&mut body).expect("connect body");
            stream
                .write_all(&[0x20, 0x02, 0x00, 0x05])
                .expect("auth rejected connack");
            let mut disconnect = [0u8; 2];
            stream.read_exact(&mut disconnect).expect("disconnect");
            assert_eq!(disconnect, [0xe0, 0x00]);
        });

        let outcome = probe_mqtt(addr, Duration::from_millis(250)).expect("outcome");

        assert_eq!(outcome.confidence, CONFIRMED);
        assert_eq!(outcome.source, "mqtt_connack_auth_rejected");
        assert!(outcome.auth_required);
        assert!(outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("requires credentials")));
        handle.join().expect("join");
    }
}
