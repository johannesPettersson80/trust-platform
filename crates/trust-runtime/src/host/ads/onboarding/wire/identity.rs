use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::errors::{OnboardingWireError, OnboardingWireErrorKind};

use super::{tc_version_string, DEFAULT_ADS_PLC_PORT};

#[cfg(target_os = "windows")]
mod windows_native;

const AMS_ROUTER_OPEN_PORT: [u8; 8] = [0, 16, 2, 0, 0, 0, 0, 0];
const AMS_ROUTER_OPEN_REPLY_HEADER: [u8; 6] = [0, 16, 8, 0, 0, 0];

pub(super) fn identify_target(
    target_ip: &str,
    router_timeout: Duration,
) -> Result<TargetIdentity, OnboardingWireError> {
    identify_target_with_native(
        target_ip,
        || windows_native_identity(target_ip),
        || local_router_identity(target_ip, router_timeout),
        || udp_identity(target_ip),
    )
}

#[cfg(test)]
fn identify_target_with<RouterProbe, UdpIdentify>(
    target_ip: &str,
    router_probe: RouterProbe,
    udp_identify: UdpIdentify,
) -> Result<TargetIdentity, OnboardingWireError>
where
    RouterProbe: FnOnce() -> Result<TargetIdentity, OnboardingWireError>,
    UdpIdentify: FnOnce() -> Result<TargetIdentity, OnboardingWireError>,
{
    identify_target_with_native(target_ip, || None, router_probe, udp_identify)
}

fn identify_target_with_native<NativeProbe, RouterProbe, UdpIdentify>(
    target_ip: &str,
    native_probe: NativeProbe,
    router_probe: RouterProbe,
    udp_identify: UdpIdentify,
) -> Result<TargetIdentity, OnboardingWireError>
where
    NativeProbe: FnOnce() -> Option<Result<TargetIdentity, OnboardingWireError>>,
    RouterProbe: FnOnce() -> Result<TargetIdentity, OnboardingWireError>,
    UdpIdentify: FnOnce() -> Result<TargetIdentity, OnboardingWireError>,
{
    let mut local_errors = Vec::new();
    if is_loopback_target(target_ip) {
        if let Some(result) = native_probe() {
            match result {
                Ok(identity) => return Ok(identity),
                Err(error) => local_errors.push(error.detail),
            }
        }
        match router_probe() {
            Ok(identity) => return Ok(identity),
            Err(error) => local_errors.push(error.detail),
        }
    }

    udp_identify().map_err(|mut error| {
        if !local_errors.is_empty() {
            error.detail = format!("{}; {}", error.detail, local_errors.join("; "));
        }
        error
    })
}

#[cfg(target_os = "windows")]
fn windows_native_identity(target_ip: &str) -> Option<Result<TargetIdentity, OnboardingWireError>> {
    Some(windows_native::local_router_identity(target_ip))
}

#[cfg(not(target_os = "windows"))]
fn windows_native_identity(
    _target_ip: &str,
) -> Option<Result<TargetIdentity, OnboardingWireError>> {
    None
}

fn local_router_identity(
    target_ip: &str,
    timeout: Duration,
) -> Result<TargetIdentity, OnboardingWireError> {
    let address = super::first_socket_addr((target_ip, ads::PORT)).map_err(|error| {
        local_router_error(target_ip, format!("resolve local AMS Router: {error}"))
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

fn udp_identity(target_ip: &str) -> Result<TargetIdentity, OnboardingWireError> {
    let info = ads::udp::get_info((target_ip, ads::UDP_PORT)).map_err(|error| {
        OnboardingWireError::new(
            OnboardingWireErrorKind::UdpIdentifyBlocked,
            format!("ADS UDP identify failed for {target_ip}: {error}"),
        )
    })?;
    Ok(TargetIdentity {
        name: Some(info.hostname),
        ip: target_ip.to_string(),
        ams_net_id: info.netid.to_string(),
        ams_port: DEFAULT_ADS_PLC_PORT,
        tc_version: tc_version_string(info.twincat_version),
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
        format!("local AMS Router identity failed for {target_ip}: {detail}"),
    )
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    use super::*;

    fn target_identity() -> TargetIdentity {
        TargetIdentity {
            name: None,
            ip: "127.0.0.1".to_string(),
            ams_net_id: "100.67.6.217.1.1".to_string(),
            ams_port: 851,
            tc_version: None,
        }
    }

    #[test]
    fn loopback_identity_prefers_local_router_before_udp() {
        let expected = target_identity();
        let result = identify_target_with(
            "127.0.0.1",
            || Ok(expected.clone()),
            || {
                Err(OnboardingWireError::new(
                    OnboardingWireErrorKind::UdpIdentifyBlocked,
                    "UDP unavailable",
                ))
            },
        )
        .expect("loopback identity from local AMS Router");

        assert_eq!(result, expected);
    }

    #[test]
    fn loopback_identity_prefers_windows_native_router_before_tcp_and_udp() {
        let expected = target_identity();
        let result = identify_target_with_native(
            "127.0.0.1",
            || Some(Ok(expected.clone())),
            || panic!("raw TCP router probe must not run after native identity succeeds"),
            || panic!("UDP identify must not run after native identity succeeds"),
        )
        .expect("loopback identity from the Windows TwinCAT ADS API");

        assert_eq!(result, expected);
    }

    #[test]
    fn local_router_open_port_reply_provides_same_host_identity() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local router fixture");
        listener
            .set_nonblocking(true)
            .expect("make local router fixture bounded");
        let address = listener.local_addr().expect("local router fixture address");
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(1);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break Some(stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            break None;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept local router request: {error}"),
                }
            };
            let Some(ref mut stream) = stream else {
                return false;
            };
            stream
                .set_nonblocking(false)
                .expect("make accepted router fixture connection blocking");
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("bound fixture reads");
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
            true
        });

        let result = local_router_identity_at(address, "127.0.0.1", Duration::from_secs(1));
        let request_observed = server.join().expect("local router fixture");

        assert!(
            request_observed,
            "local AMS Router open-port request was not sent"
        );
        let target = result.expect("local router identity");
        assert_eq!(target.ip, "127.0.0.1");
        assert_eq!(target.ams_net_id, "100.67.6.217.1.1");
        assert_eq!(target.ams_port, 851);
        assert!(target.name.is_none());
    }
}
