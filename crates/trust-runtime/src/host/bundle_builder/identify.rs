// The identify load check of a project that is checked from its sources: `trust-runtime check` and
// `trust-runtime ide serve` build no program image (specification 11, "Identify", load).

/// Makes the identify load check of the project in `bundle_root` on the declarations its sources
/// give, as `trust-runtime run` makes it on the program it loads, so that a project `check` or
/// `ide serve` accepts is a project `run` accepts.
///
/// Nothing is refused, and `Ok` is returned, when there is nothing to check: the project has no
/// `runtime.toml` or no `[runtime.identify] output`, or its `io.toml` cannot be read, or its sources
/// cannot be read or do not compile. A compile error and an invalid `io.toml` are `trust-runtime
/// check`'s own report, and the IDE is where they are fixed; neither is a problem of the identify
/// output.
///
/// # Errors
///
/// `runtime.toml` cannot be read, or the program uses the identify output, or the output lies
/// beyond the output image (the errors of
/// [`check_identify_output`](crate::io::check_identify_output)).
pub fn refuse_identify_output_use(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> Result<(), crate::error::RuntimeError> {
    let runtime_toml = bundle_root.join("runtime.toml");
    if !runtime_toml.is_file() {
        return Ok(());
    }
    let Some(setting) = crate::config::RuntimeConfig::load(&runtime_toml)?.identify_output else {
        return Ok(());
    };
    let io_toml = bundle_root.join("io.toml");
    let io = if io_toml.is_file() {
        let Ok(io) = crate::config::IoConfig::load(&io_toml) else {
            return Ok(());
        };
        io
    } else {
        crate::config::IoConfig {
            drivers: Vec::new(),
            safe_state: crate::io::IoSafeState::default(),
        }
    };
    // The program's bindings and output image as the build lays them out from its declarations,
    // without the toolchain: `check` and `ide serve` build no image (specification 44 §5.6).
    let Ok(files) = collect_project_sources_labelled(bundle_root, sources_root) else {
        return Ok(());
    };
    let Ok(declarations) = crate::harness::CompileSession::from_sources(files).declarations() else {
        return Ok(());
    };
    let interface = declarations.io();
    let bindings: Vec<crate::io::IoAddress> = interface
        .bindings()
        .iter()
        .map(|binding| binding.address.clone())
        .collect();
    crate::io::check_identify_output(&setting, &bindings, interface.outputs().len(), &io)
}
