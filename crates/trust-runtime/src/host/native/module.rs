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
    let mut code = text
        .data()
        .map_err(|error| refuse(format!("the module's code: {error}")))?
        .to_vec();
    // An x86-64 module calls the memory and arithmetic functions it links through its global
    // offset table; those calls become direct ones and the table is left behind unread.
    let unread_got = if target == Target::X86_64 {
        relax_got(&file, &text, &mut code)?
    } else {
        None
    };
    for writable in [".data", ".bss", ".got", ".got.plt", ".tdata", ".tbss"] {
        if writable == ".got" && unread_got.is_some() {
            continue;
        }
        if section(writable).is_some_and(|section| section.size() > 0) {
            return Err(refuse(format!(
                "the module has writable data in {writable}: a program keeps its values in its areas"
            )));
        }
    }
    for relocations in [".rel.dyn", ".relr.dyn"] {
        if section(relocations).is_some_and(|section| section.size() > 0) {
            return Err(refuse(format!(
                "the module has relocations in {relocations}: its code must be position-independent"
            )));
        }
    }
    if let Some(mut relocations) = file.dynamic_relocations() {
        // Only the entries of a table nothing reads may be relocated.
        if relocations
            .any(|(offset, _)| !unread_got.as_ref().is_some_and(|got| got.contains(&offset)))
        {
            return Err(refuse(
                "the module has dynamic relocations: its code must be position-independent",
            ));
        }
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

/// `R_X86_64_GOTPCREL`, a 32-bit PC-relative offset to an entry of the global offset table.
const R_X86_64_GOTPCREL: u32 = 9;

/// Relaxes every load of an address from the global offset table in the x86-64 `code` of `text`,
/// as the linker relaxes `R_X86_64_GOTPCRELX` (System V AMD64 ABI, §B.2): the compiler emits the
/// relocation it cannot relax for the calls of the functions the module links outside its
/// link-time optimization (specification 44 §4.5), which the module defines itself. A `call` and a
/// `jmp` through the table become direct, a `mov` from it a `lea`. Returns the addresses of the
/// table's entries, which nothing reads after, or `None` when the code reads none.
///
/// # Errors
///
/// [`NativeError::Module`] for an entry of a symbol the module does not define or an instruction
/// that is no `call`, `jmp` or `mov`.
fn relax_got(
    file: &object::File<'_>,
    text: &object::Section<'_, '_>,
    code: &mut [u8],
) -> Result<Option<std::ops::Range<u64>>, NativeError> {
    let mut relaxed = false;
    for (offset, relocation) in text.relocations() {
        if !matches!(
            relocation.flags(),
            object::RelocationFlags::Elf { r_type } if r_type == R_X86_64_GOTPCREL
        ) {
            continue;
        }
        let object::RelocationTarget::Symbol(index) = relocation.target() else {
            return Err(refuse("a global offset table entry of no symbol"));
        };
        let symbol = file
            .symbol_by_index(index)
            .map_err(|error| refuse(format!("a global offset table entry: {error}")))?;
        let name = symbol.name().unwrap_or("?");
        if symbol.is_undefined() {
            return Err(refuse(format!(
                "the module calls {name}, which it does not define"
            )));
        }
        let at = usize::try_from(offset).map_err(|_| refuse("a relocation beyond the code"))?;
        let (Some(first), Some(second)) = (
            at.checked_sub(2).and_then(|start| code.get(start).copied()),
            at.checked_sub(1).and_then(|start| code.get(start).copied()),
        ) else {
            return Err(refuse("a relocation at the start of the code"));
        };
        let target = i64::try_from(symbol.address()).unwrap_or(i64::MAX);
        let here = i64::try_from(offset).unwrap_or(i64::MAX);
        let displacement = |next: i64| {
            i32::try_from(target - next)
                .map(i32::to_le_bytes)
                .map_err(|_| refuse("a relaxed call beyond 2 GiB"))
        };
        match (first, second) {
            // call *sym@GOTPCREL(%rip) -> addr32 call sym
            (0xFF, 0x15) => {
                code[at - 2] = 0x67;
                code[at - 1] = 0xE8;
                code[at..at + 4].copy_from_slice(&displacement(here + 4)?);
            }
            // jmp *sym@GOTPCREL(%rip) -> jmp sym; nop
            (0xFF, 0x25) => {
                code[at - 2] = 0xE9;
                code[at - 1..at + 3].copy_from_slice(&displacement(here + 3)?);
                code[at + 3] = 0x90;
            }
            // mov sym@GOTPCREL(%rip), %reg -> lea sym(%rip), %reg
            (0x8B, modrm) if modrm & 0xC7 == 0x05 => {
                code[at - 2] = 0x8D;
                code[at..at + 4].copy_from_slice(&displacement(here + 4)?);
            }
            _ => {
                return Err(refuse(format!(
                    "the module reads the address of {name} through its global offset table in an \
                     instruction the build does not relax"
                )));
            }
        }
        relaxed = true;
    }
    if !relaxed {
        return Ok(None);
    }
    Ok(file
        .section_by_name(".got")
        .map(|got| got.address()..got.address() + got.size()))
}
