//! Writing a toolchain bundle (specification 44 §4.6) from a development toolchain folder: the
//! compiler and its libraries from the compiler's own folder, the parts of `rust-std` a module
//! links, the program library of each module target, and the manifest, packed into one archive.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::write::GzEncoder;
use flate2::Compression;

use super::{
    file_sha256, platform, relative, Manifest, ManifestFile, MANIFEST_FILE, MANIFEST_FORMAT,
    MODULE_TARGETS, TRUST_VERSION,
};
use crate::native::{triple, NativeError};

/// The parts of `rust-std` a freestanding module links, by the start of their file names.
const STD_PARTS: [&str; 2] = ["libcore-", "libcompiler_builtins-"];

/// Writes the bundle of this computer's platform into `archive` from the development toolchain
/// folder `folder` (`scripts/native/build_toolchain_libraries.sh` with every module target), and
/// returns its manifest. The compiler is the folder's `bin/rustc`, or else `rustc` on the path,
/// rustup's pinned release: a Windows folder has none, since `rustc.exe` runs only beside its own
/// libraries.
///
/// # Errors
///
/// [`NativeError::Toolchain`] when the folder's compiler cannot be asked for its folder, a part of
/// the bundle is missing (a module target's libraries, the compiler's linker), the libraries name
/// different program libraries, or a file cannot be read or written.
pub fn write_bundle(folder: &Path, archive: &Path) -> Result<Manifest, NativeError> {
    let platform = platform().ok_or_else(|| {
        NativeError::Toolchain(String::from(
            "the release builds no toolchain for this computer's platform",
        ))
    })?;
    let rustc = Some(folder.join("bin").join(executable("rustc")))
        .filter(|rustc| rustc.is_file())
        .unwrap_or_else(|| PathBuf::from("rustc"));
    let sysroot = PathBuf::from(ask(&rustc, &["--print", "sysroot"])?.trim());
    let description = ask(&rustc, &["-vV"])?;
    let field = |name: &str| {
        description
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .map(|value| value.trim().to_string())
            .ok_or_else(|| NativeError::Toolchain(format!("{} names no {name}", rustc.display())))
    };
    let host = field("host:")?;
    let release = field("release:")?;

    let mut parts: Vec<(String, PathBuf)> = vec![
        (
            format!("bin/{}", executable("rustc")),
            sysroot.join("bin").join(executable("rustc")),
        ),
        (
            format!("bin/{}", executable("rust-lld")),
            sysroot
                .join("lib/rustlib")
                .join(&host)
                .join("bin")
                .join(executable("rust-lld")),
        ),
    ];
    parts.extend(compiler_libraries(&sysroot)?);
    let mut program_library: Option<String> = None;
    for target in MODULE_TARGETS {
        let triple = triple(target);
        let std_folder = sysroot.join("lib/rustlib").join(triple).join("lib");
        let std_parts = files_in(&std_folder, |name| {
            STD_PARTS.iter().any(|part| name.starts_with(part))
        })?;
        if std_parts.len() < STD_PARTS.len() {
            return Err(NativeError::Toolchain(format!(
                "{} lacks the core of {triple}: rustup target add {triple}",
                std_folder.display()
            )));
        }
        for (name, path) in std_parts {
            parts.push((format!("lib/rustlib/{triple}/lib/{name}"), path));
        }
        let libraries = folder.join("lib").join(triple);
        let library = std::fs::read_to_string(libraries.join("LIBRARY")).map_err(|error| {
            NativeError::Toolchain(format!(
                "{}: {error}: build the libraries of every module target with \
                 scripts/native/build_toolchain_libraries.sh",
                libraries.display()
            ))
        })?;
        let library = library.trim().to_string();
        match &program_library {
            Some(first) if *first != library => {
                return Err(NativeError::Toolchain(format!(
                    "the libraries name {first} and {library}"
                )));
            }
            _ => program_library = Some(library),
        }
        for (name, path) in files_in(&libraries, |_| true)? {
            parts.push((format!("lib/{triple}/{name}"), path));
        }
    }
    parts.sort();

    let mut files = Vec::with_capacity(parts.len());
    for (path, source) in &parts {
        let (sha256, bytes) = file_sha256(source)?;
        files.push(ManifestFile {
            path: path.clone(),
            bytes,
            sha256,
            executable: path.starts_with("bin/") && !path.ends_with(".dll"),
        });
    }
    let manifest = Manifest {
        format: MANIFEST_FORMAT,
        trust: TRUST_VERSION.to_string(),
        platform: platform.to_string(),
        rustc: release,
        program_library: program_library.unwrap_or_default(),
        targets: MODULE_TARGETS
            .iter()
            .map(|target| triple(*target).to_string())
            .collect(),
        files,
    };
    pack(&manifest, &parts, archive)?;
    Ok(manifest)
}

/// Packs `manifest` and the files `parts`, each by its path in the bundle and its source, into the
/// gzip-compressed tar archive `archive`, the manifest first, every entry owned by no one and dated
/// 0, so that the same files give the same archive.
///
/// # Errors
///
/// [`NativeError::Toolchain`] when a file cannot be read or the archive written.
pub fn pack(
    manifest: &Manifest,
    parts: &[(String, PathBuf)],
    archive: &Path,
) -> Result<(), NativeError> {
    let failed =
        |error: std::io::Error| NativeError::Toolchain(format!("{}: {error}", archive.display()));
    let file = File::create(archive).map_err(failed)?;
    let mut tar = tar::Builder::new(GzEncoder::new(file, Compression::default()));
    let text = serde_json::to_vec_pretty(manifest)
        .map_err(|error| NativeError::Toolchain(format!("the manifest: {error}")))?;
    append(&mut tar, MANIFEST_FILE, &text, false).map_err(failed)?;
    for (path, source) in parts {
        if relative(Path::new(path)).as_deref() != Some(path.as_str()) {
            return Err(NativeError::Toolchain(format!(
                "{path} is not a path below the bundle"
            )));
        }
        let bytes = std::fs::read(source)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", source.display())))?;
        let executable = manifest
            .files
            .iter()
            .any(|file| file.path == *path && file.executable);
        append(&mut tar, path, &bytes, executable).map_err(failed)?;
    }
    tar.into_inner()
        .and_then(GzEncoder::finish)
        .map_err(failed)?;
    Ok(())
}

fn append(
    tar: &mut tar::Builder<GzEncoder<File>>,
    path: &str,
    bytes: &[u8],
    executable: bool,
) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(if executable { 0o755 } else { 0o644 });
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_entry_type(tar::EntryType::Regular);
    tar.append_data(&mut header, path, bytes)
}

/// `name` with this computer's suffix of programs, `.exe` on Windows.
fn executable(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// The output of `rustc` with `args`.
fn ask(rustc: &Path, args: &[&str]) -> Result<String, NativeError> {
    let output = Command::new(rustc)
        .args(args)
        .output()
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", rustc.display())))?;
    if !output.status.success() {
        return Err(NativeError::Toolchain(format!(
            "{} {}: {}",
            rustc.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The libraries the compiler loads: `lib/*.so*` on Linux, `lib/*.dylib` on macOS, `bin/*.dll` on
/// Windows.
fn compiler_libraries(sysroot: &Path) -> Result<Vec<(String, PathBuf)>, NativeError> {
    let (folder, wanted): (&str, fn(&str) -> bool) = if cfg!(windows) {
        ("bin", |name| name.ends_with(".dll"))
    } else if cfg!(target_os = "macos") {
        ("lib", |name| name.ends_with(".dylib"))
    } else {
        ("lib", |name| name.contains(".so"))
    };
    let found = files_in(&sysroot.join(folder), wanted)?;
    if found.is_empty() {
        return Err(NativeError::Toolchain(format!(
            "{} holds none of the compiler's libraries",
            sysroot.join(folder).display()
        )));
    }
    Ok(found
        .into_iter()
        .map(|(name, path)| (format!("{folder}/{name}"), path))
        .collect())
}

/// The files directly in `folder` whose names `wanted` takes, with their paths, following links.
fn files_in(
    folder: &Path,
    wanted: impl Fn(&str) -> bool,
) -> Result<Vec<(String, PathBuf)>, NativeError> {
    let entries = std::fs::read_dir(folder)
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", folder.display())))?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", folder.display())))?;
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if path.is_file() && wanted(&name) {
            found.push((name, path));
        }
    }
    found.sort();
    Ok(found)
}
