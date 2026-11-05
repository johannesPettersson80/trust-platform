//! The web IDE's reference performance gates (specification 11, 6.8.1e), in a test program of their
//! own so that they measure the web IDE alone. Inside the web IDE's integration tests they also
//! measured the tests running beside them, and on CI's runners they crossed their budgets at random:
//! the 800 ms completion maximum (28 October 2026) and the 250 ms maximum of an HTTP apply, which took
//! 323 ms (5 November 2026).
#![allow(dead_code, unused_imports)]

include!("web_ide_integration/helpers.rs");

/// Held by each gate while it measures, so that the gates take turns and neither measures the other.
static MEASURING: Mutex<()> = Mutex::new(());

#[path = "web_ide_integration/web_ide_integration_part_03.rs"]
mod web_ide_integration_part_03;
#[path = "web_ide_integration/web_ide_integration_part_04.rs"]
mod web_ide_integration_part_04;
