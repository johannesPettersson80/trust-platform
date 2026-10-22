//! The pinned Rust toolchain that compiles a generated crate into a module (specification 44 §4).
//!
//! A toolchain is a folder: the compiler's sysroot with `rustc` and `rust-lld` and the standard
//! library of every target of §4.4, and the program library and interface crates built for each
//! target with the module flags (`lib/<target>/`). The tools find it at `TRUST_TOOLCHAIN`, and a
//! development checkout uses rustup's pinned compiler with the libraries
//! `scripts/native/build_toolchain_libraries.sh` builds.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::NativeError;
use trust_runtime_core::native::image::Target;

/// The Rust target triple of a module for `target` (specification 44 §4.4): one freestanding module
/// per CPU, which every operating system's kernel of that CPU runs.
#[must_use]
pub fn triple(target: Target) -> &'static str {
    match target {
        Target::Rv32imac => "riscv32imac-unknown-none-elf",
        Target::ThumbV8mMainHf => "thumbv8m.main-none-eabihf",
        Target::X86_64 => "x86_64-unknown-linux-gnu",
        Target::Aarch64 => "aarch64-unknown-linux-gnu",
    }
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

impl Toolchain {
    /// The toolchain at `TRUST_TOOLCHAIN`: `rustc` and `rust-lld` under its `bin` folder and the
    /// libraries under its `lib` folder.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when the variable is unset or the folder lacks a part.
    pub fn from_env() -> Result<Self, NativeError> {
        let Some(root) = std::env::var_os("TRUST_TOOLCHAIN").map(PathBuf::from) else {
            return Err(NativeError::Toolchain(
                "TRUST_TOOLCHAIN does not name a toolchain folder".into(),
            ));
        };
        Self::at(&root)
    }

    /// The toolchain in folder `root`.
    ///
    /// # Errors
    ///
    /// [`NativeError::Toolchain`] when the folder lacks a part.
    pub fn at(root: &Path) -> Result<Self, NativeError> {
        let rustc = root.join("bin").join("rustc");
        let linker = root.join("bin").join("rust-lld");
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
            .arg("-C")
            .arg(format!("target-cpu={cpu}"))
            .arg("-C")
            .arg(format!("target-feature={features}"))
            .args(["-C", "remark=prolog-epilog"])
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
