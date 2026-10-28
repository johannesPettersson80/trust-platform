use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

#[derive(Debug, Default)]
pub struct ModbusTestState {
    pub coils: Vec<bool>,
    pub discrete_inputs: Vec<bool>,
    pub input_registers: Vec<u16>,
    pub holding_registers: Vec<u16>,
    pub functions: Vec<u8>,
}

impl ModbusTestState {
    pub fn with_registers(input_registers: Vec<u16>, holding_registers: Vec<u16>) -> Self {
        Self {
            input_registers,
            holding_registers,
            ..Self::default()
        }
    }
}

pub fn start_modbus_server(state: Arc<Mutex<ModbusTestState>>, requests: usize) -> SocketAddr {
    start_modbus_server_on("127.0.0.1:0", state, requests)
}

/// The server of `start_modbus_server` listening on `bind`, such as `[::1]:0` for the IPv6
/// loopback.
pub fn start_modbus_server_on(
    bind: &str,
    state: Arc<Mutex<ModbusTestState>>,
    requests: usize,
) -> SocketAddr {
    let listener = TcpListener::bind(bind)
        .unwrap_or_else(|error| panic!("bind modbus test server on {bind}: {error}"));
    let addr = listener.local_addr().expect("server addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept modbus");
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
        for _ in 0..requests {
            if handle_modbus_request(&mut stream, &state).is_err() {
                break;
            }
        }
    });
    addr
}

pub fn start_delayed_modbus_server(
    state: Arc<Mutex<ModbusTestState>>,
    response_delay: StdDuration,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed modbus test server");
    let addr = listener.local_addr().expect("server addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept delayed modbus");
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
        thread::sleep(response_delay);
        let _ = handle_modbus_request(&mut stream, &state);
    });
    addr
}

/// A server that answers the first `answered` requests of one connection, then reads the next one
/// and holds its answer, not yet applied to `state`, until the test sends on the returned channel;
/// it then answers it and the requests after it until the connection closes. Dropping the sender
/// instead closes the connection with the request unanswered.
pub fn start_withholding_modbus_server(
    state: Arc<Mutex<ModbusTestState>>,
    answered: usize,
) -> (SocketAddr, mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind withholding modbus test server");
    let addr = listener.local_addr().expect("server addr");
    let (release, released) = mpsc::channel::<()>();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept withholding modbus");
        let _ = stream.set_read_timeout(Some(StdDuration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(StdDuration::from_secs(2)));
        for _ in 0..answered {
            if handle_modbus_request(&mut stream, &state).is_err() {
                return;
            }
        }
        let held = handle_modbus_request_before_response(&mut stream, &state, || {
            released.recv().map_err(|_| ())
        });
        if held.is_ok() {
            while handle_modbus_request(&mut stream, &state).is_ok() {}
        }
    });
    (addr, release)
}

/// A server that answers `answered` requests of its first connection and then closes it, as a
/// server that goes away, and says so on the returned channel; it then takes the next connection
/// and answers its requests until that one closes.
pub fn start_closing_once_modbus_server(
    state: Arc<Mutex<ModbusTestState>>,
    answered: usize,
) -> (SocketAddr, mpsc::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind closing-once modbus test server");
    let addr = listener.local_addr().expect("server addr");
    let (closed, closes) = mpsc::channel::<()>();
    thread::spawn(move || {
        let (mut first, _) = listener.accept().expect("accept the first connection");
        let _ = first.set_read_timeout(Some(StdDuration::from_secs(2)));
        for _ in 0..answered {
            if handle_modbus_request(&mut first, &state).is_err() {
                return;
            }
        }
        drop(first);
        let _ = closed.send(());
        let (mut second, _) = listener.accept().expect("accept the next connection");
        let _ = second.set_read_timeout(Some(StdDuration::from_secs(2)));
        while handle_modbus_request(&mut second, &state).is_ok() {}
    });
    (addr, closes)
}

pub fn start_closing_modbus_server(connection_count: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind closing modbus test server");
    let addr = listener.local_addr().expect("server addr");
    thread::spawn(move || {
        for stream in listener.incoming().take(64) {
            if stream.is_err() {
                break;
            }
            connection_count.fetch_add(1, Ordering::SeqCst);
        }
    });
    addr
}

fn handle_modbus_request(
    stream: &mut TcpStream,
    state: &Arc<Mutex<ModbusTestState>>,
) -> Result<(), ()> {
    handle_modbus_request_before_response(stream, state, || Ok(()))
}

pub(super) fn handle_modbus_request_before_response(
    stream: &mut TcpStream,
    state: &Arc<Mutex<ModbusTestState>>,
    before_response: impl FnOnce() -> Result<(), ()>,
) -> Result<(), ()> {
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
    {
        let mut guard = state.lock().expect("modbus state lock");
        guard.functions.push(function);
    }
    before_response()?;
    let response = match function {
        0x01 | 0x02 => handle_read_bits(function, pdu, state),
        0x03 | 0x04 => handle_read_registers(function, pdu, state),
        0x05 => handle_write_single_coil(pdu, state),
        0x06 => handle_write_single_register(pdu, state),
        0x0F => handle_write_multiple_coils(pdu, state),
        0x10 => handle_write_multiple_registers(pdu, state),
        _ => vec![function | 0x80, 0x01],
    };
    let mut resp_header = [0u8; 6];
    resp_header[0..2].copy_from_slice(&tx.to_be_bytes());
    resp_header[2..4].copy_from_slice(&0u16.to_be_bytes());
    resp_header[4..6].copy_from_slice(&((response.len() + 1) as u16).to_be_bytes());
    // One write, as a server sends a response: three small writes left the last two to Nagle's
    // algorithm, which held them until the client's delayed acknowledgement, about 40 ms on Linux,
    // longer than the scan's deadline for a refresh of several requests.
    let mut frame = resp_header.to_vec();
    frame.push(unit_id);
    frame.extend_from_slice(&response);
    stream.write_all(&frame).map_err(|_| ())?;
    stream.flush().ok();
    Ok(())
}

fn handle_read_bits(function: u8, pdu: &[u8], state: &Arc<Mutex<ModbusTestState>>) -> Vec<u8> {
    if pdu.len() < 5 {
        return vec![function | 0x80, 0x03];
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
    let guard = state.lock().expect("modbus state lock");
    let bits = match function {
        0x01 => &guard.coils,
        0x02 => &guard.discrete_inputs,
        _ => unreachable!("read bits called with unsupported function"),
    };
    if start + qty > bits.len() {
        return vec![function | 0x80, 0x02];
    }
    let packed = pack_bits(&bits[start..start + qty]);
    let mut payload = Vec::with_capacity(2 + packed.len());
    payload.push(function);
    payload.push(packed.len() as u8);
    payload.extend_from_slice(&packed);
    payload
}

fn handle_read_registers(function: u8, pdu: &[u8], state: &Arc<Mutex<ModbusTestState>>) -> Vec<u8> {
    if pdu.len() < 5 {
        return vec![function | 0x80, 0x03];
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
    let guard = state.lock().expect("modbus state lock");
    let regs = match function {
        0x03 => &guard.holding_registers,
        0x04 => &guard.input_registers,
        _ => unreachable!("read registers called with unsupported function"),
    };
    if start + qty > regs.len() {
        return vec![function | 0x80, 0x02];
    }
    let mut payload = Vec::with_capacity(2 + qty * 2);
    payload.push(function);
    payload.push((qty * 2) as u8);
    for reg in &regs[start..start + qty] {
        payload.push((reg >> 8) as u8);
        payload.push(*reg as u8);
    }
    payload
}

fn handle_write_single_coil(pdu: &[u8], state: &Arc<Mutex<ModbusTestState>>) -> Vec<u8> {
    if pdu.len() < 5 {
        return vec![0x85, 0x03];
    }
    let address = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let value = u16::from_be_bytes([pdu[3], pdu[4]]);
    let mut guard = state.lock().expect("modbus state lock");
    if address >= guard.coils.len() {
        return vec![0x85, 0x02];
    }
    guard.coils[address] = value == 0xFF00;
    pdu[..5].to_vec()
}

fn handle_write_single_register(pdu: &[u8], state: &Arc<Mutex<ModbusTestState>>) -> Vec<u8> {
    if pdu.len() < 5 {
        return vec![0x86, 0x03];
    }
    let address = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let value = u16::from_be_bytes([pdu[3], pdu[4]]);
    let mut guard = state.lock().expect("modbus state lock");
    if address >= guard.holding_registers.len() {
        return vec![0x86, 0x02];
    }
    guard.holding_registers[address] = value;
    pdu[..5].to_vec()
}

fn handle_write_multiple_coils(pdu: &[u8], state: &Arc<Mutex<ModbusTestState>>) -> Vec<u8> {
    if pdu.len() < 6 {
        return vec![0x8F, 0x03];
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
    let byte_count = pdu[5] as usize;
    if pdu.len() < 6 + byte_count {
        return vec![0x8F, 0x03];
    }
    let mut guard = state.lock().expect("modbus state lock");
    if start + qty > guard.coils.len() {
        return vec![0x8F, 0x02];
    }
    for idx in 0..qty {
        let byte = pdu[6 + idx / 8];
        guard.coils[start + idx] = byte & (1 << (idx % 8)) != 0;
    }
    vec![0x0F, pdu[1], pdu[2], pdu[3], pdu[4]]
}

fn handle_write_multiple_registers(pdu: &[u8], state: &Arc<Mutex<ModbusTestState>>) -> Vec<u8> {
    if pdu.len() < 6 {
        return vec![0x90, 0x03];
    }
    let start = u16::from_be_bytes([pdu[1], pdu[2]]) as usize;
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
    let byte_count = pdu[5] as usize;
    if pdu.len() < 6 + byte_count {
        return vec![0x90, 0x03];
    }
    let mut guard = state.lock().expect("modbus state lock");
    if start + qty > guard.holding_registers.len() {
        return vec![0x90, 0x02];
    }
    for idx in 0..qty {
        let offset = 6 + idx * 2;
        let hi = pdu.get(offset).copied().unwrap_or(0);
        let lo = pdu.get(offset + 1).copied().unwrap_or(0);
        guard.holding_registers[start + idx] = u16::from_be_bytes([hi, lo]);
    }
    vec![0x10, pdu[1], pdu[2], pdu[3], pdu[4]]
}

fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut packed = vec![0u8; bits.len().div_ceil(8)];
    for (idx, bit) in bits.iter().enumerate() {
        if *bit {
            packed[idx / 8] |= 1 << (idx % 8);
        }
    }
    packed
}
