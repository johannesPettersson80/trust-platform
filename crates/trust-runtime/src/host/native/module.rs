//! The linked module, checked and taken apart for the image (specification 44 §5.3, §8.6): its
//! code from address 0, its read-only data at their linked distance, the offset of every entry
//! point; a module with a relocation, writable data or the panic machinery is refused.

use std::collections::HashMap;

use object::{Object, ObjectSection, ObjectSymbol};

use super::NativeError;
use trust_runtime_core::native::image::{CodeImage, Target};

/// A checked module.
pub(super) struct Module {
    pub(super) code: CodeImage,
    /// The offset from the start of the code of each entry point, by its symbol.
    pub(super) entries: HashMap<String, u32>,
}

fn refuse(reason: impl Into<String>) -> NativeError {
    NativeError::Module(reason.into())
}

/// Checks the ELF module `elf` built for `target` and takes out its code, read-only data and the
/// entry points named `entries`.
///
/// # Errors
///
/// [`NativeError::Module`] naming what the module must not have.
pub(super) fn extract(
    elf: &[u8],
    target: Target,
    entries: &[String],
) -> Result<Module, NativeError> {
    let file =
        object::File::parse(elf).map_err(|error| refuse(format!("not an ELF module: {error}")))?;
    let architecture = match target {
        Target::Rv32imac => object::Architecture::Riscv32,
        Target::ThumbV8mMainHf => object::Architecture::Arm,
        Target::X86_64 => object::Architecture::X86_64,
        Target::Aarch64 => object::Architecture::Aarch64,
    };
    if file.architecture() != architecture {
        return Err(refuse(format!(
            "the module is for {:?}, not {architecture:?}",
            file.architecture()
        )));
    }
    let section = |name: &str| file.section_by_name(name);
    let Some(text) = section(".text") else {
        return Err(refuse("the module has no code"));
    };
    if text.address() != 0 {
        return Err(refuse("the module's code does not start at address 0"));
    }
    for writable in [".data", ".bss", ".got", ".got.plt", ".tdata", ".tbss"] {
        if section(writable).is_some_and(|section| section.size() > 0) {
            return Err(refuse(format!(
                "the module has writable data in {writable}: a program keeps its values in its areas"
            )));
        }
    }
    for relocations in [".rela.dyn", ".rel.dyn", ".relr.dyn"] {
        if section(relocations).is_some_and(|section| section.size() > 0) {
            return Err(refuse(format!(
                "the module has relocations in {relocations}: its code must be position-independent"
            )));
        }
    }
    if file
        .dynamic_relocations()
        .is_some_and(|mut relocations| relocations.next().is_some())
    {
        return Err(refuse("the module has dynamic relocations"));
    }
    let panics: Vec<String> = file
        .symbols()
        .filter_map(|symbol| symbol.name().ok().map(str::to_string))
        .filter(|name| name.contains("9panicking") || name.contains("panic_bounds_check"))
        .collect();
    if !panics.is_empty() {
        return Err(refuse(format!(
            "the module can panic (specification 44 §8.6): {}",
            panics.join(", ")
        )));
    }
    let code = text
        .data()
        .map_err(|error| refuse(format!("the module's code: {error}")))?
        .to_vec();
    let (rodata, distance) = match section(".rodata") {
        Some(rodata) if rodata.size() > 0 => (
            rodata
                .data()
                .map_err(|error| refuse(format!("the module's read-only data: {error}")))?
                .to_vec(),
            rodata.address(),
        ),
        _ => (Vec::new(), (code.len() as u64).div_ceil(16) * 16),
    };
    if distance < code.len() as u64 {
        return Err(refuse("the module's read-only data overlaps its code"));
    }
    let rodata_distance = u32::try_from(distance).map_err(|_| refuse("the module is too large"))?;
    let mut found = HashMap::new();
    for symbol in file.symbols() {
        let Ok(name) = symbol.name() else {
            continue;
        };
        if entries.iter().any(|entry| entry == name) {
            let offset = u32::try_from(symbol.address())
                .map_err(|_| refuse("an entry point beyond 4 GiB"))?;
            found.insert(name.to_string(), offset);
        }
    }
    for entry in entries {
        if !found.contains_key(entry) {
            return Err(refuse(format!("the module has no entry point {entry}")));
        }
    }
    Ok(Module {
        code: CodeImage {
            code,
            rodata,
            rodata_distance,
        },
        entries: found,
    })
}
