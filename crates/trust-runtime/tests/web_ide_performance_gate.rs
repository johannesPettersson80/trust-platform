//! The web IDE's reference performance gates (specification 11), in a test program of their own so
//! that they measure the web IDE alone. Inside the web IDE's integration tests they also measured the
//! 33 tests running beside them, and on CI's small runners they crossed the 800 ms budget at random
//! (28 October 2026).
#![allow(dead_code, unused_imports)]

include!("web_ide_integration/helpers.rs");

#[path = "web_ide_integration/web_ide_integration_part_04.rs"]
mod web_ide_integration_part_04;
