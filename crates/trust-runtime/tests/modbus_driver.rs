use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use trust_runtime::io::{IoDriver, ModbusTcpDriver};

fn start_modbus_server(regs: Arc<Mutex<Vec<u16>>>, requests: usize) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind modbus test server");
    let addr = listener.local_addr().expect("server addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept modbus");
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
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
fn modbus_driver_reads_and_writes() {
    let regs = Arc::new(Mutex::new(vec![0u16; 4]));
    {
        let mut guard = regs.lock().expect("regs lock");
        guard[0] = 0x1122;
        guard[1] = 0x3344;
    }
    let addr = start_modbus_server(regs.clone(), 2);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 4];
    driver.read_inputs(&mut inputs).expect("read inputs");
    assert_eq!(inputs, vec![0x11, 0x22, 0x33, 0x44]);

    let outputs = vec![0xAA, 0xBB, 0xCC, 0xDD];
    driver.write_outputs(&outputs).expect("write outputs");
    let guard = regs.lock().expect("regs lock");
    assert_eq!(guard[0], 0xAABB);
    assert_eq!(guard[1], 0xCCDD);
}

#[test]
fn modbus_driver_warn_policy_reports_transport_error() {
    let params: toml::Value = toml::from_str(
        "address = \"127.0.0.1:65000\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\non_error = \"warn\"\n",
    )
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];
    let result = driver.read_inputs(&mut inputs);
    assert!(result.is_err(), "warn policy should report runtime error");
    let health = driver.health();
    match health {
        trust_runtime::io::IoDriverHealth::Degraded { .. } => {}
        other => panic!("expected degraded health, got {other:?}"),
    }
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn modbus_warn_policy_transport_failure_still_returns_error() {
    let params: toml::Value = toml::from_str(
        "address = \"127.0.0.1:65000\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\non_error = \"warn\"\n",
    )
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];

    let err = driver
        .read_inputs(&mut inputs)
        .expect_err("warn policy must not turn transport failure into success");
    assert!(
        err.to_string().contains("connect") || err.to_string().contains("transport"),
        "expected transport/connect error, got {err}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn modbus_exception_is_not_reported_as_generic_transport() {
    let regs = Arc::new(Mutex::new(vec![0u16; 1]));
    let addr = start_modbus_server(regs, 1);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 7\noutput_start = 0\n"
    ))
    .expect("params");
    let mut driver = ModbusTcpDriver::from_params(&params).expect("driver");
    let mut inputs = vec![0u8; 2];

    let err = driver
        .read_inputs(&mut inputs)
        .expect_err("Modbus exception must be observable");
    let text = err.to_string();
    assert!(
        text.contains("exception"),
        "expected Modbus exception, got {err}"
    );
    assert!(
        !text.starts_with("i/o driver error"),
        "Modbus exception should not use the generic transport error variant: {err}"
    );
}
