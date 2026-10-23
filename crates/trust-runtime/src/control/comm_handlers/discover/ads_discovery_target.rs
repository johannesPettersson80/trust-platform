pub(super) fn directed_target(explicit_host: Option<&str>) -> Option<String> {
    explicit_host
        .map(ToString::to_string)
        .or_else(|| Some("127.0.0.1".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_ads_host_remains_the_directed_target() {
        assert_eq!(
            directed_target(Some("192.168.77.11")),
            Some("192.168.77.11".to_string())
        );
    }

    #[test]
    fn default_ads_scan_also_targets_this_computer() {
        assert_eq!(directed_target(None), Some("127.0.0.1".to_string()));
    }
}
