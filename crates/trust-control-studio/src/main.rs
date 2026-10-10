//! The `trust-control-studio` program: Control Studio's engine (specification 35, §0.1a and §0.1b).
//! `serve` answers the extension's JSON-RPC methods, and the verbs run from a terminal, a script or
//! CI. The logic is in the library (`trust_control_studio::cli`); this file passes the arguments,
//! the environment and the standard streams on.

use std::process::ExitCode;

fn main() -> ExitCode {
    let environment = |name: &str| std::env::var_os(name);
    trust_control_studio::cli::run(
        std::env::args_os(),
        &environment,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .into()
}
