use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use trust_ads_core::AdsRoute;

use super::source_policy::ads_source_candidates_for_route;
use super::AdsRsTimeouts;
use crate::ads::transport::AdsTransportError;

const ROUTER_NEGOTIATION_TIMEOUT: Duration = Duration::from_millis(500);

/// Connects through a local AMS router when one answers, while preserving direct
/// AMS/TCP test servers such as pyads on the same loopback protocol port.
pub(super) fn connect_ads_client<A>(
    address: A,
    timeouts: AdsRsTimeouts,
    route: &AdsRoute,
) -> Result<ads::Client, AdsTransportError>
where
    A: ToSocketAddrs,
{
    let address = address
        .to_socket_addrs()
        .map_err(|error| super::map_io_error("resolve ADS server address", error))?
        .next()
        .ok_or_else(|| AdsTransportError::new("ADS server address resolved to no socket"))?;
    let sources = ads_source_candidates_for_route(route)?;
    let primary = sources
        .first()
        .copied()
        .ok_or_else(|| AdsTransportError::new("ADS source policy returned no candidate"))?;
    let Some(direct_fallback) = sources.get(1).copied() else {
        return ads::Client::new(address, timeouts.into_ads(), primary)
            .map_err(super::map_ads_error);
    };

    match detect_loopback_server_kind(address, timeouts)? {
        LoopbackServerKind::Router => {
            ads::Client::new(address, timeouts.into_ads(), primary).map_err(super::map_ads_error)
        }
        LoopbackServerKind::Direct => {
            ads::Client::new(address, timeouts.into_ads(), direct_fallback)
                .map_err(super::map_ads_error)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopbackServerKind {
    Router,
    Direct,
}

fn detect_loopback_server_kind(
    address: SocketAddr,
    timeouts: AdsRsTimeouts,
) -> Result<LoopbackServerKind, AdsTransportError> {
    let mut stream = TcpStream::connect_timeout(&address, bounded_timeout(timeouts.connect))
        .map_err(|error| super::map_io_error("connect local ADS router probe", error))?;
    stream
        .set_read_timeout(Some(bounded_timeout(timeouts.read)))
        .map_err(|error| super::map_io_error("set local ADS router probe read timeout", error))?;
    stream
        .set_write_timeout(Some(bounded_timeout(timeouts.write)))
        .map_err(|error| super::map_io_error("set local ADS router probe write timeout", error))?;

    const OPEN_PORT: [u8; 8] = [0, 16, 2, 0, 0, 0, 0, 0];
    if stream.write_all(&OPEN_PORT).is_err() {
        return Ok(LoopbackServerKind::Direct);
    }
    let mut reply = [0_u8; 14];
    if let Err(error) = stream.read_exact(&mut reply) {
        return if is_direct_server_probe_failure(&error) {
            Ok(LoopbackServerKind::Direct)
        } else {
            Err(super::map_io_error(
                "read local AMS router probe reply",
                error,
            ))
        };
    }
    if reply[..6] != [0, 16, 8, 0, 0, 0] {
        return Ok(LoopbackServerKind::Direct);
    }

    let close_port = [1, 0, 2, 0, 0, 0, reply[12], reply[13]];
    stream.write_all(&close_port).map_err(|error| {
        super::map_io_error("close temporary local AMS router probe port", error)
    })?;
    Ok(LoopbackServerKind::Router)
}

fn bounded_timeout(configured: Option<Duration>) -> Duration {
    configured.map_or(ROUTER_NEGOTIATION_TIMEOUT, |value| {
        value.min(ROUTER_NEGOTIATION_TIMEOUT)
    })
}

fn is_direct_server_probe_failure(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
    )
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    use trust_ads_core::{AdsSecurityPolicy, AmsNetId, TransportSecurity};

    use super::*;

    #[test]
    fn refused_loopback_probe_preserves_structured_connection_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve local port");
        let address = listener.local_addr().expect("reserved address");
        drop(listener);

        let error = detect_loopback_server_kind(address, AdsRsTimeouts::default())
            .expect_err("closed local port must refuse the probe");

        assert_eq!(
            error.failure_kind(),
            Some(crate::ads::AdsTransportFailureKind::ConnectionRefused)
        );
    }

    #[test]
    fn direct_loopback_server_falls_back_after_bounded_router_probe() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind direct ADS test server");
        let address = listener.local_addr().expect("direct ADS test address");
        let server = thread::spawn(move || {
            let (mut probe, _) = listener.accept().expect("accept router probe");
            let mut router_request = [0_u8; 8];
            probe
                .read_exact(&mut router_request)
                .expect("read router probe request");
            assert_eq!(router_request, [0, 16, 2, 0, 0, 0, 0, 0]);
            // A direct pyads-style server can keep an incomplete router request open.
            // Hold it well beyond the production detection timeout.
            thread::sleep(Duration::from_secs(2));
            drop(probe);

            let (direct, _) = listener.accept().expect("accept direct ADS fallback");
            drop(direct);
        });
        let route = AdsRoute {
            name: "pyads-direct".to_string(),
            target_net_id: AmsNetId::new("127.0.0.1.1.1"),
            host: "127.0.0.1".to_string(),
            ams_port: 851,
            local_net_id: Some(AmsNetId::new("127.0.0.1.1.1")),
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: false,
            },
        };

        let started = Instant::now();
        let client = connect_ads_client(
            address,
            AdsRsTimeouts {
                connect: Some(Duration::from_secs(3)),
                read: Some(Duration::from_secs(3)),
                write: Some(Duration::from_secs(3)),
            },
            &route,
        )
        .expect("direct loopback ADS server fallback");
        assert!(
            started.elapsed() < Duration::from_millis(1_500),
            "direct-server fallback must not wait for the server to close the router probe"
        );
        drop(client);
        server.join().expect("direct ADS test server");
    }
}
