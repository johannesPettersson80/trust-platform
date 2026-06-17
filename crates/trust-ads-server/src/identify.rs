//! Beckhoff UDP discovery/control responder for ADS server onboarding.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::AmsNetIdBytes;

const BECKHOFF_UDP_MAGIC: u32 = 0x7114_6603;
const SERVICE_IDENTIFY: u32 = 1;
const SERVICE_ADD_ROUTE: u32 = 6;
const SERVICE_REPLY_FLAG: u32 = 0x8000_0000;
const UDP_HEADER_LEN: usize = 24;
const TAG_STATUS: u16 = 1;
const TAG_TC_VERSION: u16 = 3;
const TAG_COMPUTER_NAME: u16 = 5;
const TAG_NET_ID: u16 = 7;

/// UDP identify responder configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdsIdentifyConfig {
    /// UDP bind address, normally `<runtime-host-ip>:48899`.
    pub bind_addr: SocketAddr,
    /// AMS Net ID reported by the runtime.
    pub local_net_id: AmsNetIdBytes,
    /// AMS source port reported in the UDP header.
    pub source_port: u16,
    /// Host/device name reported to discovery clients.
    pub hostname: String,
    /// TwinCAT-compatible version tuple reported by discovery.
    pub twincat_version: (u8, u8, u16),
    /// Maximum UDP datagram bytes accepted.
    pub max_packet_bytes: usize,
}

/// Running ADS UDP identify responder.
pub struct AdsIdentifyServer {
    local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl AdsIdentifyServer {
    /// Starts the UDP identify responder.
    ///
    /// # Errors
    ///
    /// Returns a listener error when the UDP socket cannot bind or configure.
    pub fn start(config: AdsIdentifyConfig) -> Result<Self, AdsIdentifyError> {
        let socket = UdpSocket::bind(config.bind_addr).map_err(AdsIdentifyError::Bind)?;
        socket
            .set_nonblocking(true)
            .map_err(AdsIdentifyError::Configure)?;
        let local_addr = socket.local_addr().map_err(AdsIdentifyError::Configure)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let join = thread::Builder::new()
            .name("trust-runtime-ads-identify".to_string())
            .spawn(move || run_identify_loop(socket, config, stop_thread))
            .map_err(AdsIdentifyError::Spawn)?;
        Ok(Self {
            local_addr,
            stop,
            join: Some(join),
        })
    }

    /// Returns the actual UDP bind address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Requests shutdown and joins the responder thread.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for AdsIdentifyServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Error returned by the ADS UDP identify responder.
#[derive(Debug)]
pub enum AdsIdentifyError {
    /// Failed to bind the socket.
    Bind(io::Error),
    /// Failed to configure the socket.
    Configure(io::Error),
    /// Failed to spawn the responder thread.
    Spawn(io::Error),
}

impl core::fmt::Display for AdsIdentifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bind(err) => write!(f, "failed to bind ADS UDP identify responder: {err}"),
            Self::Configure(err) => {
                write!(f, "failed to configure ADS UDP identify responder: {err}")
            }
            Self::Spawn(err) => write!(f, "failed to spawn ADS UDP identify responder: {err}"),
        }
    }
}

impl std::error::Error for AdsIdentifyError {}

#[allow(clippy::needless_pass_by_value)]
fn run_identify_loop(socket: UdpSocket, config: AdsIdentifyConfig, stop: Arc<AtomicBool>) {
    let packet_cap = config.max_packet_bytes.clamp(UDP_HEADER_LEN, 4096);
    let mut buf = vec![0_u8; packet_cap];
    while !stop.load(Ordering::SeqCst) {
        match socket.recv_from(&mut buf) {
            Ok((len, peer)) => {
                let request = &buf[..len];
                if is_identify_request(request) {
                    let response = build_identify_response(&config);
                    let _ = socket.send_to(&response, peer);
                } else if is_add_route_request(request) {
                    let response = build_add_route_response(&config);
                    let _ = socket.send_to(&response, peer);
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn is_identify_request(bytes: &[u8]) -> bool {
    is_udp_request(bytes, SERVICE_IDENTIFY)
}

fn is_add_route_request(bytes: &[u8]) -> bool {
    is_udp_request(bytes, SERVICE_ADD_ROUTE)
}

fn is_udp_request(bytes: &[u8], service: u32) -> bool {
    if bytes.len() < UDP_HEADER_LEN {
        return false;
    }
    read_u32(bytes, 0) == Some(BECKHOFF_UDP_MAGIC) && read_u32(bytes, 8) == Some(service)
}

fn build_identify_response(config: &AdsIdentifyConfig) -> Vec<u8> {
    let mut out = udp_header(config, SERVICE_IDENTIFY | SERVICE_REPLY_FLAG);
    out.reserve(72);
    add_u32(&mut out, TAG_STATUS, 0);
    add_bytes(&mut out, TAG_NET_ID, &config.local_net_id);
    add_str(&mut out, TAG_COMPUTER_NAME, &config.hostname);
    add_bytes(
        &mut out,
        TAG_TC_VERSION,
        &tc_version_bytes(config.twincat_version),
    );
    let item_count = 4_u32;
    out[20..24].copy_from_slice(&item_count.to_le_bytes());
    out
}

fn build_add_route_response(config: &AdsIdentifyConfig) -> Vec<u8> {
    let mut out = udp_header(config, SERVICE_ADD_ROUTE | SERVICE_REPLY_FLAG);
    add_u32(&mut out, TAG_STATUS, 0);
    out[20..24].copy_from_slice(&1_u32.to_le_bytes());
    out
}

fn udp_header(config: &AdsIdentifyConfig, service: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(UDP_HEADER_LEN);
    out.extend_from_slice(&BECKHOFF_UDP_MAGIC.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&service.to_le_bytes());
    out.extend_from_slice(&config.local_net_id);
    out.extend_from_slice(&config.source_port.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out
}

fn add_bytes(out: &mut Vec<u8>, tag: u16, bytes: &[u8]) {
    let Ok(len) = u16::try_from(bytes.len()) else {
        return;
    };
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

fn add_str(out: &mut Vec<u8>, tag: u16, value: &str) {
    let truncated = truncate_to_u16_len_with_nul(value.as_bytes());
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(
        &u16::try_from(truncated.len())
            .expect("truncate_to_u16_len_with_nul guarantees u16 length")
            .to_le_bytes(),
    );
    out.extend_from_slice(&truncated);
}

fn add_u32(out: &mut Vec<u8>, tag: u16, value: u32) {
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&4_u16.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

fn truncate_to_u16_len_with_nul(bytes: &[u8]) -> Vec<u8> {
    let max_without_nul = usize::from(u16::MAX) - 1;
    let mut out = bytes[..bytes.len().min(max_without_nul)].to_vec();
    out.push(0);
    out
}

fn tc_version_bytes(version: (u8, u8, u16)) -> [u8; 4] {
    let (major, minor, build) = version;
    let mut out = [major, minor, 0, 0];
    out[2..4].copy_from_slice(&build.to_le_bytes());
    out
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;
    use std::time::Duration;

    use super::{
        AdsIdentifyConfig, AdsIdentifyServer, BECKHOFF_UDP_MAGIC, SERVICE_ADD_ROUTE,
        SERVICE_IDENTIFY, SERVICE_REPLY_FLAG, TAG_COMPUTER_NAME, TAG_NET_ID, TAG_STATUS,
        TAG_TC_VERSION,
    };

    #[test]
    fn identify_responder_replies_with_runtime_identity_tags() {
        let mut server = AdsIdentifyServer::start(AdsIdentifyConfig {
            bind_addr: "127.0.0.1:0".parse().expect("bind addr"),
            local_net_id: [127, 0, 0, 1, 1, 1],
            source_port: 0,
            hostname: "trust-runtime".to_string(),
            twincat_version: (3, 1, 4026),
            max_packet_bytes: 512,
        })
        .expect("start identify");
        let client = UdpSocket::bind("127.0.0.1:0").expect("client bind");
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");

        client
            .send_to(&identify_request(), server.local_addr())
            .expect("send identify");
        let mut response = [0_u8; 512];
        let (len, _) = client.recv_from(&mut response).expect("identify response");

        assert_eq!(read_u32(&response[..len], 0), BECKHOFF_UDP_MAGIC);
        assert_eq!(
            read_u32(&response[..len], 8),
            SERVICE_IDENTIFY | SERVICE_REPLY_FLAG
        );
        assert_eq!(&response[12..18], &[127, 0, 0, 1, 1, 1]);
        assert_eq!(
            tag(&response[..len], TAG_NET_ID).expect("net id"),
            &[127, 0, 0, 1, 1, 1]
        );
        assert_eq!(
            tag(&response[..len], TAG_COMPUTER_NAME).expect("name"),
            b"trust-runtime\0"
        );
        assert_eq!(
            tag(&response[..len], TAG_TC_VERSION).expect("tc version"),
            &[3, 1, 0xBA, 0x0F]
        );
        server.shutdown();
    }

    #[test]
    fn route_add_responder_acks_tcat_route_workflow_without_granting_access() {
        let mut server = AdsIdentifyServer::start(AdsIdentifyConfig {
            bind_addr: "127.0.0.1:0".parse().expect("bind addr"),
            local_net_id: [127, 0, 0, 1, 1, 1],
            source_port: 0,
            hostname: "trust-runtime".to_string(),
            twincat_version: (3, 1, 4026),
            max_packet_bytes: 512,
        })
        .expect("start identify");
        let client = UdpSocket::bind("127.0.0.1:0").expect("client bind");
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");

        client
            .send_to(&route_add_request(), server.local_addr())
            .expect("send add route");
        let mut response = [0_u8; 512];
        let (len, _) = client.recv_from(&mut response).expect("add route response");

        assert_eq!(read_u32(&response[..len], 0), BECKHOFF_UDP_MAGIC);
        assert_eq!(
            read_u32(&response[..len], 8),
            SERVICE_ADD_ROUTE | SERVICE_REPLY_FLAG
        );
        assert_eq!(&response[12..18], &[127, 0, 0, 1, 1, 1]);
        assert_eq!(
            tag(&response[..len], TAG_STATUS).expect("status"),
            &0_u32.to_le_bytes()
        );
        server.shutdown();
    }

    fn identify_request() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&BECKHOFF_UDP_MAGIC.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&SERVICE_IDENTIFY.to_le_bytes());
        out.extend_from_slice(&[0_u8; 6]);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out
    }

    fn route_add_request() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&BECKHOFF_UDP_MAGIC.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&SERVICE_ADD_ROUTE.to_le_bytes());
        out.extend_from_slice(&[192, 168, 77, 11, 1, 1]);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        add_test_bytes(&mut out, TAG_NET_ID, &[192, 168, 77, 11, 1, 1]);
        add_test_str(&mut out, TAG_COMPUTER_NAME, "192.168.77.11");
        add_test_str(&mut out, 13, "Administrator");
        add_test_str(&mut out, 2, "1");
        add_test_str(&mut out, 12, "TwinCAT-engineering");
        out[20..24].copy_from_slice(&5_u32.to_le_bytes());
        out
    }

    fn add_test_bytes(out: &mut Vec<u8>, tag: u16, bytes: &[u8]) {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(
            &u16::try_from(bytes.len())
                .expect("test tag len")
                .to_le_bytes(),
        );
        out.extend_from_slice(bytes);
    }

    fn add_test_str(out: &mut Vec<u8>, tag: u16, value: &str) {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        add_test_bytes(out, tag, &bytes);
    }

    fn tag(bytes: &[u8], wanted: u16) -> Option<&[u8]> {
        let mut offset = 24;
        while offset + 4 <= bytes.len() {
            let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().ok()?);
            let len = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().ok()?) as usize;
            let start = offset + 4;
            let end = start.checked_add(len)?;
            if end > bytes.len() {
                return None;
            }
            if tag == wanted {
                return Some(&bytes[start..end]);
            }
            offset = end;
        }
        None
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
    }
}
