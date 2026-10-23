pub fn interface_directed_targets<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut targets = Vec::new();
    for candidate in candidates {
        let Ok(address) = candidate.trim().parse::<std::net::Ipv4Addr>() else {
            continue;
        };
        if address.is_unspecified()
            || address.is_loopback()
            || address.is_link_local()
            || address.is_multicast()
            || address == std::net::Ipv4Addr::BROADCAST
        {
            continue;
        }
        let target = address.to_string();
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ads_scan_directs_identify_to_usable_local_ipv4_addresses() {
        assert_eq!(
            interface_directed_targets([
                "127.0.0.1",
                "::1",
                "169.254.2.9",
                "192.168.77.11",
                "100.67.6.217",
                "0.0.0.0",
                "224.0.0.1",
                "192.168.77.11",
            ]),
            vec!["192.168.77.11".to_string(), "100.67.6.217".to_string(),]
        );
    }
}
