//! Runtime Cloud sites (specification 11, "Runtime Cloud sites"): the site a runtime belongs to,
//! and the ID by which a runtime names another, relative to its own site.

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

/// The site of a runtime whose configured site is `site`: that site, or the default site.
#[must_use]
pub fn runtime_cloud_site_or_default(site: Option<&str>) -> &str {
    site.unwrap_or(RUNTIME_CLOUD_DEFAULT_SITE)
}

/// The ID by which a runtime of the site `own_site` names the resource `resource` of the site
/// `site`: the resource name when both are in one site, and `<site>/<resource>` otherwise.
#[must_use]
pub fn runtime_cloud_id(own_site: &str, site: &str, resource: &str) -> String {
    if site == own_site {
        resource.to_string()
    } else {
        format!("{site}/{resource}")
    }
}

/// The site that `id`, as a runtime of the site `own_site` names a runtime, names: the part
/// before its `/`, or `own_site` for a bare ID.
#[must_use]
pub fn runtime_cloud_site_of<'a>(id: &'a str, own_site: &'a str) -> &'a str {
    match id.split_once('/') {
        Some((site, runtime)) if !site.trim().is_empty() && !runtime.trim().is_empty() => {
            site.trim()
        }
        _ => own_site,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Catches a site setting that takes a value the keyspace and the IDs cannot carry, or that
    /// gives `default-site` a site of its own.
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
        assert_eq!(runtime_cloud_site_or_default(None), "default-site");
        assert_eq!(runtime_cloud_site_or_default(Some("site-b")), "site-b");
    }

    /// Catches an ID that qualifies a runtime of the naming runtime's own site, which would make
    /// every reference inside a project change when the project is given a site, or that loses
    /// the site of a runtime of another site.
    #[test]
    fn a_runtime_names_its_own_sites_runtimes_by_resource_and_others_by_site() {
        assert_eq!(
            runtime_cloud_id("default-site", "default-site", "runtime_a"),
            "runtime_a"
        );
        assert_eq!(
            runtime_cloud_id("site-b", "site-b", "runtime_c"),
            "runtime_c"
        );
        assert_eq!(
            runtime_cloud_id("default-site", "site-b", "runtime_b"),
            "site-b/runtime_b"
        );
        assert_eq!(
            runtime_cloud_id("site-b", "default-site", "runtime_a"),
            "default-site/runtime_a"
        );
    }

    /// Catches a bare ID read in another site than the naming runtime's own.
    #[test]
    fn an_id_names_its_site_and_a_bare_id_the_own_site() {
        assert_eq!(
            runtime_cloud_site_of("site-b/runtime_b", "site-a"),
            "site-b"
        );
        assert_eq!(runtime_cloud_site_of("runtime_c", "site-a"), "site-a");
        assert_eq!(
            runtime_cloud_site_of("runtime_a", "default-site"),
            "default-site"
        );
        assert_eq!(runtime_cloud_site_of("/runtime_a", "site-a"), "site-a");
        assert_eq!(runtime_cloud_site_of("site-b/", "site-a"), "site-a");
    }
}
