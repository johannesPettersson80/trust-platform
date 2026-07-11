use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};

use super::{DirectedIdentityObservation, DirectedIdentityTransport, DEFAULT_ADS_PLC_PORT};

const AMS_ROUTER_OPEN_PORT: [u8; 8] = [0, 16, 2, 0, 0, 0, 0, 0];
const AMS_ROUTER_OPEN_REPLY_HEADER: [u8; 6] = [0, 16, 8, 0, 0, 0];

pub(super) fn identify_target(
    target_ip: &str,
    router_timeout: Duration,
) -> Result<DirectedIdentityObservation, OnboardingWireError> {
    let mut router_error = None;
    if is_loopback_target(target_ip) {
        match local_router_identity(target_ip, router_timeout) {
            Ok(target) => {
                return Ok(DirectedIdentityObservation {
                    target,
                    transport: DirectedIdentityTransport::LocalRouter,
                })
            }
            Err(error) => router_error = Some(error),
        }
    }

    let info = ads::udp::get_info((target_ip, ads::UDP_PORT)).map_err(|error| {
        let router_detail = router_error
            .map(|router_error| format!("; local AMS router fallback failed: {router_error}"))
            .unwrap_or_default();
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("ADS UDP identify failed for {target_ip}: {error}{router_detail}"),
        )
    })?;
    Ok(DirectedIdentityObservation {
        target: TargetIdentity {
            name: Some(info.hostname),
            ip: target_ip.to_string(),
            ams_net_id: info.netid.to_string(),
            ams_port: DEFAULT_ADS_PLC_PORT,
            tc_version: tc_version_string(info.twincat_version),
        },
        transport: DirectedIdentityTransport::Udp,
    })
}

pub(super) fn identify_all(
    target_ip: &str,
    receive_window: Duration,
) -> Result<Vec<TargetIdentity>, OnboardingWireError> {
    let request = ads::udp::Message::new(ads::udp::ServiceId::Identify, ads::AmsAddr::default());
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| {
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("bind ADS UDP identify socket: {error}"),
        )
    })?;
    socket.set_broadcast(true).map_err(|error| {
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("enable ADS UDP broadcast: {error}"),
        )
    })?;
    socket
        .set_read_timeout(Some(receive_window))
        .map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                format!("set ADS UDP identify timeout: {error}"),
            )
        })?;
    socket
        .send_to(request.as_bytes(), (target_ip, ads::UDP_PORT))
        .map_err(|error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                format!("send ADS UDP identify to {target_ip}: {error}"),
            )
        })?;

    let mut results = Vec::new();
    let mut reply = [0_u8; 576];
    loop {
        match socket.recv_from(&mut reply) {
            Ok((len, peer)) => {
                let Ok(message) =
                    ads::udp::Message::parse(&reply[..len], ads::udp::ServiceId::Identify, true)
                else {
                    continue;
                };
                let identity = target_identity_from_udp_message(&message, peer);
                if !results.iter().any(|existing: &TargetIdentity| {
                    existing.ip == identity.ip || existing.ams_net_id == identity.ams_net_id
                }) {
                    results.push(identity);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(error) => {
                return Err(OnboardingWireError::new(
                    OnboardingWireErrorKind::UdpIdentifyBlocked,
                    format!("receive ADS UDP identify replies: {error}"),
                ));
            }
        }
    }

    if results.is_empty() {
        Err(OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("ADS UDP identify failed for {target_ip}: no target answered"),
        ))
    } else {
        Ok(results)
    }
}

fn local_router_identity(
    target_ip: &str,
    timeout: Duration,
) -> Result<TargetIdentity, OnboardingWireError> {
    let address = first_socket_addr((target_ip, ads::PORT)).map_err(|error| {
        local_router_error(target_ip, format!("resolve local AMS router: {error}"))
    })?;
    local_router_identity_at(address, target_ip, timeout)
}

fn local_router_identity_at(
    address: SocketAddr,
    target_ip: &str,
    timeout: Duration,
) -> Result<TargetIdentity, OnboardingWireError> {
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| local_router_error(target_ip, format!("connect: {error}")))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| local_router_error(target_ip, format!("set read timeout: {error}")))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| local_router_error(target_ip, format!("set write timeout: {error}")))?;
    stream
        .write_all(&AMS_ROUTER_OPEN_PORT)
        .map_err(|error| local_router_error(target_ip, format!("request identity: {error}")))?;

    let mut reply = [0_u8; 14];
    stream
        .read_exact(&mut reply)
        .map_err(|error| local_router_error(target_ip, format!("read identity: {error}")))?;
    if reply[..6] != AMS_ROUTER_OPEN_REPLY_HEADER || reply[6..12].iter().all(|byte| *byte == 0) {
        return Err(local_router_error(
            target_ip,
            "unexpected open-port reply".to_string(),
        ));
    }

    let close_port = [1, 0, 2, 0, 0, 0, reply[12], reply[13]];
    stream
        .write_all(&close_port)
        .map_err(|error| local_router_error(target_ip, format!("close identity port: {error}")))?;
    Ok(TargetIdentity {
        name: None,
        ip: target_ip.to_string(),
        ams_net_id: ams_net_id_text(&reply[6..12]),
        ams_port: DEFAULT_ADS_PLC_PORT,
        tc_version: None,
    })
}

fn target_identity_from_udp_message(
    message: &ads::udp::Message,
    peer: SocketAddr,
) -> TargetIdentity {
    TargetIdentity {
        name: message
            .get_str(ads::udp::Tag::ComputerName)
            .filter(|name| !name.is_empty())
            .map(ToString::to_string),
        ip: peer.ip().to_string(),
        ams_net_id: message.get_source().netid().to_string(),
        ams_port: DEFAULT_ADS_PLC_PORT,
        tc_version: message
            .get_bytes(ads::udp::Tag::TCVersion)
            .and_then(parse_tc_version_tag)
            .and_then(tc_version_string),
    }
}

fn parse_tc_version_tag(bytes: &[u8]) -> Option<(u8, u8, u16)> {
    (bytes.len() >= 4).then(|| (bytes[0], bytes[1], u16::from_le_bytes([bytes[2], bytes[3]])))
}

fn tc_version_string(version: (u8, u8, u16)) -> Option<String> {
    if version == (0, 0, 0) {
        None
    } else {
        Some(format!("{}.{}.{}", version.0, version.1, version.2))
    }
}

fn first_socket_addr(target: (&str, u16)) -> Result<SocketAddr, std::io::Error> {
    target.to_socket_addrs()?.next().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no socket address resolved",
        )
    })
}

fn is_loopback_target(target: &str) -> bool {
    let normalized = target.trim().trim_end_matches('.');
    normalized.eq_ignore_ascii_case("localhost")
        || normalized
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn ams_net_id_text(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn local_router_error(target_ip: &str, detail: String) -> OnboardingWireError {
    OnboardingWireError::new(
        OnboardingWireErrorKind::UdpIdentifyBlocked,
        format!("local AMS router identity failed for {target_ip}: {detail}"),
    )
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn local_router_open_port_reply_provides_same_computer_identity() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local router fixture");
        let address = listener.local_addr().expect("local router fixture address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept local router request");
            let mut open = [0_u8; 8];
            stream
                .read_exact(&mut open)
                .expect("read open-port request");
            assert_eq!(open, AMS_ROUTER_OPEN_PORT);
            stream
                .write_all(&[0, 16, 8, 0, 0, 0, 100, 67, 6, 217, 1, 1, 0x21, 0xC3])
                .expect("write router identity reply");
            let mut close = [0_u8; 8];
            stream
                .read_exact(&mut close)
                .expect("read close-port request");
            assert_eq!(close, [1, 0, 2, 0, 0, 0, 0x21, 0xC3]);
        });

        let target = local_router_identity_at(address, "127.0.0.1", Duration::from_secs(1))
            .expect("local router identity");

        assert_eq!(target.ip, "127.0.0.1");
        assert_eq!(target.ams_net_id, "100.67.6.217.1.1");
        assert_eq!(target.ams_port, 851);
        assert!(target.name.is_none());
        server.join().expect("local router fixture");
    }
}
