//! Which functions of the C6 firmware run from RAM (`.rwtext`), judged by each function's own module
//! (checklist G217; `firmware/trust-esp32c6/ld/rwtext_hook.x`). The linker list holds the scan path:
//! the runner's control path, its stepped scans and its program, stays in flash (the firmware's
//! README, "RAM"). A pattern of the list matches a path anywhere in a symbol, a generic argument's
//! too, so a module whose types the control path takes as arguments drew that path into RAM; the
//! check reads the module that a function belongs to, not the modules its arguments come from.

/// The modules of the runner's control path, which run between scans and stay in flash: its stepped
/// scans and debug reads (`runner::stepping`) and its program (`runner::program`).
pub(super) const CONTROL_PATH: [&str; 2] = [
    "trust_runtime_services::runner::stepping",
    "trust_runtime_services::runner::program",
];

/// The crate and the modules a v0-mangled symbol (`_R…`) names first: the module of the function
/// itself, or of the impl it is a method of. A symbol of another scheme has none.
pub(super) fn own_module(symbol: &str) -> Option<String> {
    let mangled = symbol.strip_prefix("_R")?;
    // The first crate root is `C`, its disambiguator `s<base 62>_`, and its name; the names of the
    // modules below it follow, each prefixed with its length.
    let root = mangled.find("Cs")?;
    let after = &mangled[root + 2..];
    let names = &after[after.find('_')? + 1..];
    let mut parts = Vec::new();
    let mut rest = names;
    while let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
        if end == 0 {
            break;
        }
        let length: usize = rest[..end].parse().ok()?;
        let name = rest.get(end..end + length)?;
        parts.push(name);
        rest = &rest[end + length..];
    }
    (!parts.is_empty()).then(|| parts.join("::"))
}

/// The functions of `symbols` (name, address) in `ram` whose own module is in the control path.
pub(super) fn control_path_in_ram<'a>(
    symbols: impl IntoIterator<Item = (&'a str, u64)>,
    ram: &std::ops::Range<u64>,
) -> Vec<String> {
    let mut found: Vec<String> = symbols
        .into_iter()
        .filter(|(_, address)| ram.contains(address))
        .filter(|(name, _)| {
            own_module(name).is_some_and(|module| {
                CONTROL_PATH
                    .iter()
                    .any(|path| module == *path || module.starts_with(&format!("{path}::")))
            })
        })
        .map(|(name, _)| name.to_string())
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
#[path = "ram_code_tests.rs"]
mod tests;
