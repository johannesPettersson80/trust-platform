//! Test fixtures shared by the page-file tests.

pub(super) const FILE: &str = "hmi/pages/overview.toml";

/// The page example of specification 39, §15.3.
pub(super) const SPEC_EXAMPLE: &str = r#"format = 1
title = "Overview"

[[object]]
id = "tank-1"
tag = "TK101"
type = "tank"
at = [400, 300]
size = [160, 260]

[object.bind]
level = { path = "FeedArea.TK101.Level", unit = "%" }

[[object]]
id = "pump-1"
tag = "P101"
type = "pump"
variant = "horizontal"
at = [640, 480]
size = [80, 80]

[object.bind]
fault = "FeedArea.P101.Fault"
running = "FeedArea.P101.Running"
speed = { path = "FeedArea.P101.Speed", unit = "rpm" }

[object.click]
faceplate = "pump"

[[pipe]]
from = "tank-1.outlet"
to = "pump-1.inlet"
"#;
