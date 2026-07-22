pub(crate) fn canonicalize_ams_net_id(value: &str) -> Option<String> {
    let octets = decimal_ams_net_id_octets(value.trim())?;
    Some(canonical_text(octets))
}

pub(crate) fn parse_canonical_ams_net_id(value: &str) -> Option<[u8; 6]> {
    let octets = decimal_ams_net_id_octets(value)?;
    (canonical_text(octets) == value).then_some(octets)
}

pub(crate) fn is_canonical_ams_net_id(value: &str) -> bool {
    parse_canonical_ams_net_id(value).is_some()
}

fn decimal_ams_net_id_octets(value: &str) -> Option<[u8; 6]> {
    let mut parts = value.split('.');
    let mut octets = [0; 6];
    for octet in &mut octets {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        *octet = part.parse().ok()?;
    }
    parts.next().is_none().then_some(octets)
}

fn canonical_text(octets: [u8; 6]) -> String {
    format!(
        "{}.{}.{}.{}.{}.{}",
        octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
    )
}
