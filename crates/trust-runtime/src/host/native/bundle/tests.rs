//! The toolchain bundle of specification 44 §4.6: its manifest and check, and its install from a
//! file and from a release.

use std::path::{Path, PathBuf};

use super::pack::pack;
use super::*;

/// A new, empty folder of its own, removed when dropped.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-bundle-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        std::fs::create_dir_all(&path).expect("a test folder");
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The files of a small bundle: a compiler and a library's `LIBRARY`.
const FILES: [(&str, &str); 2] = [
    ("bin/rustc", "#!/bin/sh\necho rustc 1.99.0\n"),
    (
        "lib/riscv32imac-unknown-none-elf/LIBRARY",
        "trust-plc-lib 9.9.9\n",
    ),
];

/// The manifest of [`FILES`] for truST `version` on `platform`.
fn manifest(sources: &Path, version: &str, platform: &str) -> Manifest {
    Manifest {
        format: MANIFEST_FORMAT,
        trust: version.to_string(),
        platform: platform.to_string(),
        rustc: String::from("1.99.0"),
        program_library: String::from("trust-plc-lib 9.9.9"),
        targets: vec![String::from("riscv32imac-unknown-none-elf")],
        files: FILES
            .iter()
            .map(|(path, _)| {
                let (sha256, bytes) = file_sha256(&sources.join(path)).expect("a source");
                ManifestFile {
                    path: (*path).to_string(),
                    bytes,
                    sha256,
                    executable: path.starts_with("bin/"),
                }
            })
            .collect(),
    }
}

/// The sources of [`FILES`] in `folder/sources`, by their paths in the bundle.
fn sources(folder: &Path) -> (PathBuf, Vec<(String, PathBuf)>) {
    let root = folder.join("sources");
    let parts = FILES
        .iter()
        .map(|(path, text)| {
            let source = root.join(path);
            std::fs::create_dir_all(source.parent().expect("a parent")).expect("a folder");
            std::fs::write(&source, text).expect("a source");
            ((*path).to_string(), source)
        })
        .collect();
    (root, parts)
}

/// A bundle archive of [`FILES`] whose manifest is `manifest` with `change` applied.
fn archive(folder: &Path, name: &str, change: impl FnOnce(&mut Manifest)) -> (PathBuf, Manifest) {
    let (root, parts) = sources(folder);
    let mut manifest = manifest(&root, "9.9.9", "linux-x64");
    change(&mut manifest);
    let archive = folder.join(name);
    pack(&manifest, &parts, &archive).expect("the archive is packed");
    (archive, manifest)
}

fn installer(folder: &Path) -> Installer {
    Installer {
        toolchains: folder.join("toolchains"),
        version: String::from("9.9.9"),
        platform: String::from("linux-x64"),
    }
}

/// The folders an install leaves in `toolchains/`, without the lock file.
fn left(installer: &Installer) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&installer.toolchains)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name != ".lock")
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Specification 44 §4.6: a bundle file installs into `toolchains/<version>/` after its files are
/// checked against its manifest, the compiler marked executable; a second install finds it in place
/// and does nothing. Catches an install that skips the check, puts the bundle elsewhere than where
/// the tools look, leaves the compiler unrunnable, or unpacks the archive again for every build.
#[test]
fn a_bundle_installs_from_a_file_and_holds_what_its_manifest_names() {
    let folder = Folder::new("install");
    let (archive, packed) = archive(&folder.0, "bundle.tar.gz", |_| {});
    let installer = installer(&folder.0);
    let mut events = Vec::new();
    let installed = installer
        .install_file(&archive, &mut |event| events.push(event))
        .expect("the bundle installs");
    assert_eq!(installed, folder.0.join("toolchains/9.9.9"));
    assert_eq!(
        events,
        [
            Progress::Verify,
            Progress::Installed {
                folder: installed.clone()
            }
        ]
    );
    assert_eq!(verify(&installed).expect("the bundle checks"), packed);
    assert_eq!(
        std::fs::read_to_string(installed.join("lib/riscv32imac-unknown-none-elf/LIBRARY"))
            .expect("LIBRARY"),
        "trust-plc-lib 9.9.9\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(installed.join("bin/rustc"))
            .expect("rustc")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    assert_eq!(left(&installer), ["9.9.9"]);

    let mut again = Vec::new();
    installer
        .install_file(&archive, &mut |event| again.push(event))
        .expect("the bundle is in place");
    assert_eq!(again, [Progress::Installed { folder: installed }]);
}

/// Specification 44 §4.6: a bundle a file of which is not the one its manifest names, one with a
/// file its manifest does not list, one of another platform, one of another manifest format, and
/// one with a path outside the bundle are refused, and none leaves a folder behind. Catches an
/// install that trusts the archive, keeps a half-checked bundle where the tools would use it, or
/// writes outside its folder.
#[test]
fn a_bundle_that_is_not_what_it_says_installs_nothing() {
    let folder = Folder::new("refused");
    let installer = installer(&folder.0);
    let refused = |archive: &Path, reason: &str| {
        let error = installer
            .install_file(archive, &mut |_| {})
            .expect_err("the bundle is refused");
        assert!(error.to_string().contains(reason), "{error}");
        assert_eq!(left(&installer), Vec::<String>::new(), "{reason}");
    };

    let (changed, _) = archive(&folder.0, "changed.tar.gz", |manifest| {
        manifest.files[0].sha256 = "00".repeat(32);
    });
    refused(&changed, "is not the file the manifest names");

    let (extra, _) = archive(&folder.0, "extra.tar.gz", |manifest| {
        manifest.files.pop();
    });
    refused(&extra, "is not in the manifest");

    let (windows, _) = archive(&folder.0, "windows.tar.gz", |manifest| {
        manifest.platform = String::from("win32-x64");
    });
    refused(
        &windows,
        "the bundle is truST 9.9.9 for win32-x64, this is truST 9.9.9 on linux-x64",
    );

    let (format, _) = archive(&folder.0, "format.tar.gz", |manifest| {
        manifest.format = 2;
    });
    refused(&format, "has format 2, this release reads 1");

    let outside = folder.0.join("outside.tar.gz");
    {
        let file = std::fs::File::create(&outside).expect("the archive");
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        let mut header = tar::Header::new_old();
        header.as_old_mut().name[..9].copy_from_slice(b"../escape");
        header.set_size(1);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar.append(&header, &b"x"[..]).expect("the entry");
        tar.into_inner()
            .and_then(flate2::write::GzEncoder::finish)
            .expect("the archive is written");
    }
    refused(&outside, "holds a path outside the bundle");
    assert!(!folder.0.join("escape").exists());
}

/// A server of `files` by their paths below `/`, for `requests` requests, at `http://<address>/`.
fn serve(files: Vec<(String, Vec<u8>)>, requests: usize) -> (String, std::thread::JoinHandle<()>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("a server");
    let port = server.server_addr().to_ip().expect("an address").port();
    let handle = std::thread::spawn(move || {
        for _ in 0..requests {
            let Ok(request) = server.recv() else {
                return;
            };
            let found = files
                .iter()
                .find(|(path, _)| *path == request.url())
                .map(|(_, bytes)| bytes.clone());
            let _ = match found {
                Some(bytes) => request.respond(tiny_http::Response::from_data(bytes)),
                None => request.respond(tiny_http::Response::empty(404)),
            };
        }
    });
    (format!("http://127.0.0.1:{port}/"), handle)
}

/// Specification 44 §4.6: the bundle of `v<version>` is downloaded beside the release's
/// `SHA256SUMS`, whose lines name each asset with the folder the release job kept it in, installed
/// when its SHA-256 is the one the sums list, and refused when it is not or when the sums list no
/// bundle of the platform; a refused download leaves no partial file. Catches a download installed
/// unchecked, the sums of another file taken, a bundle the release lists under a folder not found,
/// and partial files left to fill the data folder.
#[test]
fn a_release_bundle_is_checked_against_the_release_sums() {
    let folder = Folder::new("download");
    let (archive, _) = archive(&folder.0, "bundle.tar.gz", |_| {});
    let bytes = std::fs::read(&archive).expect("the archive");
    let (sha256, length) = file_sha256(&archive).expect("its SHA-256");
    let name = archive_name("linux-x64");

    let wrong = Installer {
        toolchains: folder.0.join("wrong"),
        ..installer(&folder.0)
    };
    let (base, server) = serve(
        vec![
            (
                String::from("/v9.9.9/SHA256SUMS"),
                format!("{}  {name}\n", "11".repeat(32)).into_bytes(),
            ),
            (format!("/v9.9.9/{name}"), bytes.clone()),
        ],
        2,
    );
    let error = wrong
        .download(&base, &mut |_| {})
        .expect_err("another SHA-256 is refused");
    assert!(
        error.to_string().contains(&format!(
            "has the SHA-256 {sha256}, the release lists {}",
            "11".repeat(32)
        )),
        "{error}"
    );
    server.join().expect("the server");
    assert_eq!(left(&wrong), Vec::<String>::new());

    let unlisted = Installer {
        toolchains: folder.0.join("unlisted"),
        ..installer(&folder.0)
    };
    let (base, server) = serve(
        vec![(
            String::from("/v9.9.9/SHA256SUMS"),
            format!("{sha256}  trust-toolchain-win32-x64.tar.gz\n").into_bytes(),
        )],
        1,
    );
    let error = unlisted
        .download(&base, &mut |_| {})
        .expect_err("a release without the bundle is refused");
    assert!(
        error
            .to_string()
            .contains("the release v9.9.9 lists no trust-toolchain-linux-x64.tar.gz"),
        "{error}"
    );
    server.join().expect("the server");

    let installer = installer(&folder.0);
    let (base, server) = serve(
        vec![
            (
                String::from("/v9.9.9/SHA256SUMS"),
                format!(
                    "{}  runtime-artifacts/other.tar.gz\n{sha256}  toolchain-artifacts/{name}\n",
                    "22".repeat(32)
                )
                .into_bytes(),
            ),
            (format!("/v9.9.9/{name}"), bytes),
        ],
        2,
    );
    let mut events = Vec::new();
    let installed = installer
        .download(&base, &mut |event| events.push(event))
        .expect("the bundle downloads and installs");
    server.join().expect("the server");
    assert_eq!(installed, folder.0.join("toolchains/9.9.9"));
    assert_eq!(
        events.last(),
        Some(&Progress::Installed {
            folder: installed.clone()
        })
    );
    assert!(events.contains(&Progress::Download {
        bytes: length,
        total: Some(length)
    }));
    assert!(verify(&installed).is_ok());
    assert_eq!(left(&installer), ["9.9.9"]);
}

/// Specification 44 §4.6: the VS Code extension carries its platform's bundle beside its tools, and
/// the install takes that file with no download; tools that carry none download the release's
/// bundle. Catches tools that download although they carry the bundle (no server answers the
/// first install), and tools that need the carried file to install at all.
#[test]
fn a_bundle_beside_the_tools_installs_without_a_download() {
    let folder = Folder::new("beside");
    let tools = folder.0.join("bin");
    std::fs::create_dir_all(&tools).expect("the tools' folder");
    let name = archive_name("linux-x64");
    let (archive, packed) = archive(&folder.0, &format!("bin/{name}"), |_| {});

    let carried = installer(&folder.0);
    let mut events = Vec::new();
    let installed = carried
        .install(Some(&tools), "http://127.0.0.1:9/", &mut |event| {
            events.push(event);
        })
        .expect("the carried bundle installs");
    assert_eq!(installed, folder.0.join("toolchains/9.9.9"));
    assert_eq!(
        events,
        [
            Progress::Verify,
            Progress::Installed {
                folder: installed.clone()
            }
        ]
    );
    assert_eq!(verify(&installed).expect("the bundle checks"), packed);

    let downloading = Installer {
        toolchains: folder.0.join("downloaded"),
        ..installer(&folder.0)
    };
    let (sha256, _) = file_sha256(&archive).expect("its SHA-256");
    let (base, server) = serve(
        vec![
            (
                String::from("/v9.9.9/SHA256SUMS"),
                format!("{sha256}  {name}\n").into_bytes(),
            ),
            (
                format!("/v9.9.9/{name}"),
                std::fs::read(&archive).expect("the archive"),
            ),
        ],
        2,
    );
    let downloaded = downloading
        .install(Some(&folder.0.join("no-bundle")), &base, &mut |_| {})
        .expect("tools without a bundle download it");
    server.join().expect("the server");
    assert_eq!(downloaded, folder.0.join("downloaded/9.9.9"));
}
