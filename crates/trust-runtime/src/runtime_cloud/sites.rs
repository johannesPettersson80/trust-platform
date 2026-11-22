//! Runtime Cloud sites (specification 11, "Runtime Cloud sites"): the site a runtime belongs to,
//! and the cloud ID that names a runtime by its site and its resource name.

use smol_str::SmolStr;

use super::projection::RUNTIME_CLOUD_DEFAULT_SITE;

/// The longest site name, a DNS label's length.
const SITE_MAX_LEN: usize = 63;

/// The site that the value of `[runtime.cloud] site` names: `None` for the default site, which
/// `default-site` names as well, and an error for a value that is not 1 to 63 ASCII letters,
/// digits, `-` and `_` beginning with a letter or a digit.
pub fn parse_runtime_cloud_site(value: &str) -> Result<Option<SmolStr>, String> {
    let valid = !value.is_empty()
        && value.len() <= SITE_MAX_LEN
        && value.starts_with(|c: char| c.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(format!(
            "runtime.cloud.site must be 1 to {SITE_MAX_LEN} ASCII letters, digits, '-' and '_', \
             beginning with a letter or a digit, not '{value}'"
        ));
    }
    Ok((value != RUNTIME_CLOUD_DEFAULT_SITE).then(|| SmolStr::new(value)))
}

/// The cloud ID of the resource `resource` of the site `site`: the resource name in the default
/// site (`None`), and `<site>/<resource>` in any other site.
#[must_use]
pub fn runtime_cloud_id(site: Option<&str>, resource: &str) -> String {
    match site {
        Some(site) if site != RUNTIME_CLOUD_DEFAULT_SITE => format!("{site}/{resource}"),
        _ => resource.to_string(),
    }
}

/// The site that the cloud ID `cloud_id` names: the part before its `/`, or the default site for
/// a bare ID.
#[must_use]
pub fn runtime_cloud_site_of(cloud_id: &str) -> &str {
    match cloud_id.split_once('/') {
        Some((site, runtime)) if !site.trim().is_empty() && !runtime.trim().is_empty() => {
            site.trim()
        }
        _ => RUNTIME_CLOUD_DEFAULT_SITE,
    }
}

/// The resource name that the cloud ID `cloud_id` names: the part after its site, or the whole of
/// a bare ID.
#[must_use]
pub fn runtime_cloud_resource_of(cloud_id: &str) -> &str {
    match cloud_id.split_once('/') {
        Some((site, runtime)) if !site.trim().is_empty() && !runtime.trim().is_empty() => {
            runtime.trim()
        }
        _ => cloud_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Catches a site setting that takes a value the keyspace and the cloud ID cannot carry, or
    /// that gives `default-site` an ID of its own.
    #[test]
    fn a_site_is_a_short_identifier_and_default_site_is_the_default() {
        assert_eq!(
            parse_runtime_cloud_site("site-b"),
            Ok(Some(SmolStr::new("site-b")))
        );
        assert_eq!(
            parse_runtime_cloud_site("Plant_7"),
            Ok(Some(SmolStr::new("Plant_7")))
        );
        assert_eq!(parse_runtime_cloud_site("default-site"), Ok(None));
        let longest = "s".repeat(63);
        assert_eq!(
            parse_runtime_cloud_site(&longest),
            Ok(Some(SmolStr::new(&longest)))
        );
        for refused in [
            "",
            "-site",
            "_site",
            "site b",
            "site/b",
            "site*",
            " site-b",
            "sité",
            &"s".repeat(64),
        ] {
            let error = parse_runtime_cloud_site(refused).expect_err(refused);
            assert!(error.starts_with("runtime.cloud.site must be"), "{error}");
        }
    }

    /// Catches a cloud ID that qualifies a runtime of the default site, which would change every
    /// existing ID, or that loses the site of any other.
    #[test]
    fn the_cloud_id_qualifies_a_resource_outside_the_default_site() {
        assert_eq!(runtime_cloud_id(None, "runtime_a"), "runtime_a");
        assert_eq!(
            runtime_cloud_id(Some("default-site"), "runtime_a"),
            "runtime_a"
        );
        assert_eq!(
            runtime_cloud_id(Some("site-b"), "runtime_b"),
            "site-b/runtime_b"
        );
    }

    /// Catches a bare ID read as a runtime of some other site than the default site, and a
    /// resource name that keeps its site.
    #[test]
    fn a_cloud_id_names_its_site_and_a_bare_id_the_default_site() {
        assert_eq!(runtime_cloud_site_of("site-b/runtime_b"), "site-b");
        assert_eq!(runtime_cloud_site_of("runtime_a"), "default-site");
        assert_eq!(runtime_cloud_site_of("/runtime_a"), "default-site");
        assert_eq!(runtime_cloud_site_of("site-b/"), "default-site");
        assert_eq!(runtime_cloud_resource_of("site-b/runtime_b"), "runtime_b");
        assert_eq!(runtime_cloud_resource_of("runtime_a"), "runtime_a");
    }
}
