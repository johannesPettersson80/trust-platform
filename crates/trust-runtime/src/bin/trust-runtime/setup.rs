//! Setup command handler.

use std::io::IsTerminal;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use smol_str::SmolStr;

use crate::cli::{SetupAccessArg, SetupModeArg};
use crate::prompt;
use crate::style;
use crate::wizard;

mod setup_web;

include!("setup/models.rs");
include!("setup/command_entry.rs");
include!("setup/browser_flows.rs");
include!("setup/cli_flows.rs");
include!("setup/helpers.rs");

#[cfg(test)]
mod tests {
    //! Setup builds the project's program: the tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

    include!("setup/tests.rs");
}
