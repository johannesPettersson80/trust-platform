//! The pinned Rust toolchain that compiles a generated crate into a module (specification 44 §4).
//!
//! A toolchain is a folder: the compiler's sysroot with `rustc` and `rust-lld` and the standard
//! library of every target of §4.4, and the program library and interface crates built for each
//! target with the module flags (`lib/<target>/`). The tools find it at `TRUST_TOOLCHAIN`, which a
//! development checkout points at rustup's pinned compiler with the libraries
//! `scripts/native/build_toolchain_libraries.sh` builds, or else as this release's bundle in
//! truST's data folder (§4.6), which `trust-runtime` installs on the first build that needs it.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::NativeError;
use trust_runtime_core::native::image::Target;

/// The Rust target triple of a module for `target` (specification 44 §4.4): one freestanding module
/// per CPU, which every operating system's kernel of that CPU runs.
#[must_use]
pub fn triple(target: Target) -> &'static str {
    trust_runtime_core::native::library::triple(target)
}

/// The CPU and features of a module for `target`, fixed so that every device computes the same
/// bits (specification 44 §9).
fn cpu(target: Target) -> (&'static str, &'static str) {
    match target {
        Target::Rv32imac => ("generic-rv32", "+m,+a,+c"),
        Target::ThumbV8mMainHf => ("cortex-m33", "+fp-armv8d16sp"),
        Target::X86_64 => ("x86-64", "+sse2"),
        Target::Aarch64 => ("generic", "+reserve-x18"),
    }
}

/// The linker script of every module: the code from address 0, the read-only data after it, and
/// sections for data a module must not have, so that their presence is seen and refused. The tables
/// a position-independent executable has (the dynamic symbols and the dynamic section, which the
/// image does not carry) are placed after them: left to the linker, they go before the code of a
/// module without read-only data.
const LINKER_SCRIPT: &str = "SECTIONS
{
  . = 0;
  .text : ALIGN(64) { *(.text.trust_entry_*) *(.text .text.*) }
  .rodata : ALIGN(16) { *(.rodata .rodata.* .srodata .srodata.*) }
  .data : { *(.data .data.* .sdata .sdata.* .data.rel.ro .data.rel.ro.*) }
  .bss : { *(.bss .bss.* .sbss .sbss.* COMMON) }
  .dynsym : { *(.dynsym) }
  .gnu.hash : { *(.gnu.hash) }
  .hash : { *(.hash) }
  .dynstr : { *(.dynstr) }
  .dynamic : { *(.dynamic) }
  /DISCARD/ : { *(.eh_frame .eh_frame_hdr .comment .note .note.*) }
}
";

/// A toolchain.
#[derive(Clone, Debug)]
pub struct Toolchain {
    /// The compiler.
    pub rustc: PathBuf,
    /// The linker, `rust-lld` of the compiler's sysroot.
    pub linker: PathBuf,
    /// The folder of the program library built for each target: `<libraries>/<triple>/`.
    pub libraries: PathBuf,
    /// The compiler's version, for the image header.
    pub version: String,
}

/// Where the tools found their toolchain (specification 44 §4.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolchainSource {
    /// The folder `TRUST_TOOLCHAIN` names.
    Environment,
    /// This release's bundle in truST's data folder.
    Installed,
}

impl Toolchain {
    /// The toolchain the tools use (specification 44 §4.6): the folder `TRUST_TOOLCHAIN` names when
    /// it is set, else this release's bundle in truST's data folder; nothing is downloaded.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when `TRUST_TOOLCHAIN` names a folder that lacks a part, or no
    /// bundle is installed.
    pub fn find() -> Result<Self, NativeError> {
        Self::locate().map(|(toolchain, _)| toolchain)
    }

    /// [`Self::find`], with where the toolchain was found.
    ///
    /// # Errors
    ///
    /// As [`Self::find`].
    pub fn locate() -> Result<(Self, ToolchainSource), NativeError> {
        if let Some(root) = std::env::var_os("TRUST_TOOLCHAIN").map(PathBuf::from) {
            return Self::at(&root).map(|toolchain| (toolchain, ToolchainSource::Environment));
        }
        let folder = super::bundle::installed_folder()
            .filter(|folder| folder.join(super::bundle::MANIFEST_FILE).is_file())
            .ok_or_else(|| {
                NativeError::Toolchain(format!(
                    "the truST toolchain {} is not installed: `trust-runtime toolchain install` \
                     installs it, from the bundle beside the tools or downloaded (about 220 MB, \
                     once)",
                    super::bundle::TRUST_VERSION
                ))
            })?;
        Self::at(&folder).map(|toolchain| (toolchain, ToolchainSource::Installed))
    }

    /// [`Self::find`], installing this release's bundle when none is found and `TRUST_TOOLCHAIN` is
    /// unset, as `trust-runtime` does on the first build that needs it (specification 44 §4.6): the
    /// bundle the tools carry beside their program, else downloaded; `report` hears the install's
    /// progress.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when `TRUST_TOOLCHAIN` names a folder that lacks a part, or the
    /// install fails.
    pub fn find_or_install(
        report: &mut dyn FnMut(super::bundle::Progress),
    ) -> Result<Self, NativeError> {
        match Self::find() {
            Ok(toolchain) => Ok(toolchain),
            Err(_) if std::env::var_os("TRUST_TOOLCHAIN").is_none() => {
                let folder = super::bundle::install(report)?;
                Self::at(&folder)
            }
            Err(error) => Err(error),
        }
    }

    /// The toolchain in folder `root`.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when the folder lacks a part.
    pub fn at(root: &Path) -> Result<Self, NativeError> {
        let suffix = std::env::consts::EXE_SUFFIX;
        let rustc = root.join("bin").join(format!("rustc{suffix}"));
        let linker = root.join("bin").join(format!("rust-lld{suffix}"));
        for part in [&rustc, &linker] {
            if !part.is_file() {
                return Err(NativeError::Toolchain(format!(
                    "{} is missing from the toolchain",
                    part.display()
                )));
            }
        }
        let output = Command::new(&rustc)
            .arg("--version")
            .output()
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", rustc.display())))?;
        let version = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_string();
        Ok(Self {
            rustc,
            linker,
            libraries: root.join("lib"),
            version,
        })
    }

    /// The program library the toolchain holds for `target`, `trust-plc-lib <version>`, as its
    /// `LIBRARY` file names it (specification 44 §4.6, §5.2).
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when the toolchain names no program library for `target`.
    pub fn program_library(&self, target: Target) -> Result<String, NativeError> {
        let path = self.libraries.join(triple(target)).join("LIBRARY");
        let text = std::fs::read_to_string(&path).map_err(|error| {
            NativeError::Toolchain(format!(
                "{} names no program library ({error}): build its libraries again with \
                 scripts/native/build_toolchain_libraries.sh",
                path.display()
            ))
        })?;
        Ok(text.trim().to_string())
    }

    /// The program library's and the interface's compiled crates for `target`.
    fn library(&self, target: Target, name: &str) -> Result<PathBuf, NativeError> {
        let folder = self.libraries.join(triple(target));
        let prefix = format!("lib{name}");
        let entries = std::fs::read_dir(&folder)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", folder.display())))?;
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "rlib")
                    && path.file_stem().is_some_and(|stem| {
                        let stem = stem.to_string_lossy();
                        stem == prefix || stem.starts_with(&format!("{prefix}-"))
                    })
            })
            .collect();
        found.sort();
        match found.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(NativeError::Toolchain(format!(
                "{} has no {name} for {}",
                folder.display(),
                triple(target)
            ))),
            _ => Err(NativeError::Toolchain(format!(
                "{} has more than one {name}",
                folder.display()
            ))),
        }
    }

    /// Compiles the crate `source` for `target` in folder `work` and returns the linked module, an
    /// ELF file, and the compiler's report of each function's stack frame.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] with the compiler's diagnostics when it fails.
    pub fn compile(
        &self,
        source: &str,
        target: Target,
        entries: &[String],
        work: &Path,
    ) -> Result<(Vec<u8>, String), NativeError> {
        std::fs::create_dir_all(work)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", work.display())))?;
        let crate_file = work.join("module.rs");
        let script = work.join("module.x");
        let output = work.join("module.elf");
        std::fs::write(&crate_file, source)
            .and_then(|()| std::fs::write(&script, LINKER_SCRIPT))
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", work.display())))?;
        let abi = self.library(target, "trust_plc_abi")?;
        let lib = self.library(target, "trust_plc_lib")?;
        let mem = if target.is_computer() {
            Some(self.library(target, "trust_plc_mem")?)
        } else {
            None
        };
        let (cpu, features) = cpu(target);
        let mut command = Command::new(&self.rustc);
        command
            .args([
                "--edition",
                "2024",
                "--crate-type",
                "bin",
                "--crate-name",
                "trust_module",
            ])
            .args(["--target", triple(target)])
            .args([
                "-C",
                "opt-level=2",
                "-C",
                "codegen-units=1",
                "-C",
                "lto=fat",
            ])
            .args(["-C", "panic=abort", "-C", "relocation-model=pic"])
            .args(["-C", "overflow-checks=off", "-C", "debuginfo=0"])
            // A module has no dynamic linking, so read-only relocations mean nothing for it; with
            // full RELRO the compiler calls the memory functions through a global offset table
            // that a module may not have (specification 44 §4.5), without it directly.
            .args(["-C", "relro-level=off"])
            .arg("-C")
            .arg(format!("target-cpu={cpu}"))
            .arg("-C")
            .arg(format!("target-feature={features}"))
            .args(["-C", "remark=prolog-epilog"])
            // Each build has a folder of its own, whose path reaches no module, so that the same
            // project builds the same bytes (NX-07).
            .arg(format!("--remap-path-prefix={}=.", work.display()))
            .args(["--cfg", "trust_module"])
            .arg("-C")
            .arg(format!("linker={}", self.linker.display()))
            .args(["-C", "linker-flavor=ld.lld"])
            .arg("-C")
            .arg(format!("link-arg=-T{}", script.display()))
            .args(["-C", "link-arg=--pie", "-C", "link-arg=--no-dynamic-linker"])
            .args(["-C", "link-arg=-nostdlib", "-C", "link-arg=--emit-relocs"])
            .arg("-L")
            .arg(format!(
                "dependency={}",
                self.libraries.join(triple(target)).display()
            ))
            .arg("--extern")
            .arg(format!("trust_plc_abi={}", abi.display()))
            .arg("--extern")
            .arg(format!("trust_plc_lib={}", lib.display()));
        if let Some(mem) = mem {
            command
                .arg("--extern")
                .arg(format!("trust_plc_mem={}", mem.display()));
        }
        for entry in entries {
            command
                .arg("-C")
                .arg(format!("link-arg=--undefined={entry}"));
        }
        command.arg(&crate_file).arg("-o").arg(&output);
        let result = command.output().map_err(|error| {
            NativeError::Toolchain(format!("{}: {error}", self.rustc.display()))
        })?;
        let diagnostics = String::from_utf8_lossy(&result.stderr).to_string();
        if !result.status.success() {
            let errors: String = diagnostics
                .lines()
                .filter(|line| !line.starts_with("note: <unknown file>:0:0 prolog-epilog"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(NativeError::Toolchain(errors));
        }
        let module = std::fs::read(&output)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", output.display())))?;
        Ok((module, diagnostics))
    }
}

/// The stack frame of each function the compiler reported (`-C remark=prolog-epilog`), by its
/// symbol: what specification 44 §8.5's bound adds up.
#[must_use]
pub fn stack_frames(diagnostics: &str) -> Vec<(String, u64)> {
    diagnostics
        .lines()
        .filter_map(|line| {
            let rest = line.split("prolog-epilog (analysis): ").nth(1)?;
            let (bytes, function) = rest.split_once(" stack bytes in function '")?;
            let function = function.strip_suffix('\'')?;
            Some((function.to_string(), bytes.trim().parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiler's remark gives each function's frame, which the stack bound adds.
    #[test]
    fn stack_frames_are_read_from_the_remarks() {
        let remarks = "note: <unknown file>:0:0 prolog-epilog (analysis): 16 stack bytes in function 'trust_entry_p0'\n\
                       warning: unrelated\n\
                       note: <unknown file>:0:0 prolog-epilog (analysis): 0 stack bytes in function '_RNvCs_4core'\n";
        assert_eq!(
            stack_frames(remarks),
            [
                ("trust_entry_p0".to_string(), 16),
                ("_RNvCs_4core".to_string(), 0)
            ]
        );
    }
}
