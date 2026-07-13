#[cfg(any(not(windows), test))]
use std::io::{Read, Write};
#[cfg(any(not(windows), test))]
use std::net::TcpStream;
#[cfg(not(windows))]
use std::net::ToSocketAddrs;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Duration;

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};

use super::{
    DirectedIdentityObservation, DirectedIdentityTransport, ObservedAdsIdentity,
    DEFAULT_ADS_PLC_PORT,
};

#[cfg(any(windows, test))]
mod local_runtime;
#[cfg(any(windows, test))]
mod windows_runtime;

#[cfg(any(not(windows), test))]
const AMS_ROUTER_OPEN_PORT: [u8; 8] = [0, 16, 2, 0, 0, 0, 0, 0];
#[cfg(any(not(windows), test))]
const AMS_ROUTER_OPEN_REPLY_HEADER: [u8; 6] = [0, 16, 8, 0, 0, 0];
pub(super) fn identify_targets(
    target_ip: &str,
    router_timeout: Duration,
) -> Result<Vec<DirectedIdentityObservation>, OnboardingWireError> {
    #[cfg(windows)]
    if is_loopback_target(target_ip)
        || crate::ads::backend_host::target_uses_native_windows_router(target_ip)
    {
        return local_runtime::identities(target_ip, router_timeout).map(|report| {
            let mut warnings = Some(report.warnings);
            report
                .identities
                .into_iter()
                .map(|identity| DirectedIdentityObservation {
                    identity,
                    transport: DirectedIdentityTransport::LocalRouter,
                    warnings: warnings.take().unwrap_or_default(),
                })
                .collect()
        });
    }
    #[cfg(not(windows))]
    if is_loopback_target(target_ip) {
        return match identify_target(target_ip, router_timeout) {
            Ok(target) => Ok(vec![target]),
            Err(_) => Ok(Vec::new()),
        };
    }
    identify_target(target_ip, router_timeout).map(|target| vec![target])
}

pub(super) fn identify_target(
    target_ip: &str,
    router_timeout: Duration,
) -> Result<DirectedIdentityObservation, OnboardingWireError> {
    #[cfg(windows)]
    let router_error: Option<OnboardingWireError> = None;
    #[cfg(not(windows))]
    let mut router_error: Option<OnboardingWireError> = None;
    #[cfg(windows)]
    let is_local_router_target = is_loopback_target(target_ip)
        || crate::ads::backend_host::target_uses_native_windows_router(target_ip);
    #[cfg(not(windows))]
    let is_local_router_target = is_loopback_target(target_ip);
    if is_local_router_target {
        let local_result = local_router_identity(target_ip, router_timeout);
        #[cfg(windows)]
        return local_result.map(|target| DirectedIdentityObservation {
            identity: ObservedAdsIdentity::responding(target),
            transport: DirectedIdentityTransport::LocalRouter,
            warnings: Vec::new(),
        });
        #[cfg(not(windows))]
        match local_result {
            Ok(target) => {
                return Ok(DirectedIdentityObservation {
                    identity: ObservedAdsIdentity::identity_only(target),
                    transport: DirectedIdentityTransport::LocalRouter,
                    warnings: Vec::new(),
                })
            }
            Err(error) => router_error = Some(error),
        }
    }

    let info = ads::udp::get_info((target_ip, ads::UDP_PORT)).map_err(|error| {
        let router_detail = router_error
            .map(|router_error| {
                format!(
                    "; local AMS router fallback failed: {}",
                    router_error.detail
                )
            })
            .unwrap_or_default();
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("ADS UDP identify failed for {target_ip}: {error}{router_detail}"),
        )
    })?;
    Ok(DirectedIdentityObservation {
        identity: ObservedAdsIdentity::identity_only(TargetIdentity {
            name: Some(info.hostname),
            ip: target_ip.to_string(),
            ams_net_id: info.netid.to_string(),
            ams_port: DEFAULT_ADS_PLC_PORT,
            tc_version: tc_version_string(info.twincat_version),
        }),
        transport: DirectedIdentityTransport::Udp,
        warnings: Vec::new(),
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
    send_identify_requests(&socket, request.as_bytes(), (target_ip, ads::UDP_PORT)).map_err(
        |error| {
            OnboardingWireError::new(
                OnboardingWireErrorKind::UdpIdentifyBlocked,
                format!("send ADS UDP identify to {target_ip}: {error}"),
            )
        },
    )?;

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

    Ok(results)
}

fn send_identify_requests<T>(socket: &UdpSocket, request: &[u8], target: T) -> std::io::Result<()>
where
    T: std::net::ToSocketAddrs + Copy,
{
    socket.send_to(request, target)?;
    // UDP discovery is intentionally connectionless and a single request or
    // reply can be lost. One short, bounded retry materially improves the
    // normal one-button LAN journey without extending the receive window or
    // turning discovery into an unbounded scan.
    std::thread::sleep(Duration::from_millis(75));
    socket.send_to(request, target)?;
    Ok(())
}

fn local_router_identity(
    target_ip: &str,
    timeout: Duration,
) -> Result<TargetIdentity, OnboardingWireError> {
    #[cfg(windows)]
    {
        local_runtime::identities(target_ip, timeout)?
            .identities
            .into_iter()
            .find_map(ObservedAdsIdentity::into_responding_target)
            .ok_or_else(|| {
                local_router_error(
                    target_ip,
                    "a local ADS identity responded, but no user ADS service port responded"
                        .to_string(),
                )
            })
    }
    #[cfg(not(windows))]
    {
        let address = first_socket_addr((target_ip, ads::PORT)).map_err(|error| {
            local_router_error(target_ip, format!("resolve local AMS router: {error}"))
        })?;
        local_router_identity_at(address, target_ip, timeout)
    }
}

#[cfg(any(not(windows), test))]
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

#[cfg(not(windows))]
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

#[cfg(any(not(windows), test))]
fn ams_net_id_text(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn local_router_error(target_ip: &str, detail: String) -> OnboardingWireError {
    OnboardingWireError::new(
        OnboardingWireErrorKind::LocalRouterUnavailable,
        format!("local ADS router/runtime check failed for {target_ip}: {detail}"),
    )
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, UdpSocket};
    use std::thread;

    use super::*;

    #[test]
    fn lan_identify_sends_one_bounded_retry_before_waiting_for_replies() {
        let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind UDP identify receiver");
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set UDP identify receiver timeout");
        let sender = UdpSocket::bind("127.0.0.1:0").expect("bind UDP identify sender");
        let target = receiver
            .local_addr()
            .expect("UDP identify receiver address");
        let request =
            ads::udp::Message::new(ads::udp::ServiceId::Identify, ads::AmsAddr::default());

        send_identify_requests(&sender, request.as_bytes(), target)
            .expect("send bounded UDP identify attempts");

        let mut first = [0_u8; 576];
        let (first_len, _) = receiver
            .recv_from(&mut first)
            .expect("receive first attempt");
        let mut retry = [0_u8; 576];
        let (retry_len, _) = receiver
            .recv_from(&mut retry)
            .expect("receive retry attempt");
        assert_eq!(&first[..first_len], request.as_bytes());
        assert_eq!(&retry[..retry_len], request.as_bytes());
    }

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
                .write_all(&[0, 16, 8, 0, 0, 0, 10, 20, 30, 40, 1, 1, 0x21, 0xC3])
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
        assert_eq!(target.ams_net_id, "10.20.30.40.1.1");
        assert_eq!(target.ams_port, 851);
        assert!(target.name.is_none());
        server.join().expect("local router fixture");
    }

    #[test]
    fn local_router_failure_is_not_reported_as_udp_or_firewall_discovery() {
        let error = local_router_error(
            "127.0.0.1",
            "open installed TcAdsDll.dll: library not found".to_string(),
        );

        assert_eq!(error.kind, OnboardingWireErrorKind::LocalRouterUnavailable);
        assert!(error.detail.contains("local ADS router/runtime"));
        assert!(!error.to_string().contains("UdpIdentifyBlocked"));
        assert!(!error.detail.to_ascii_lowercase().contains("firewall"));
    }
}
