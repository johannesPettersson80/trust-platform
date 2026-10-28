//! The signing keys and the trusted keys of specification 44 §5.11.

use super::*;

/// A new, empty folder of its own, removed when dropped.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-signing-{name}-{}-{}",
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

/// Specification 44 §5.11: a new key is an Ed25519 key in PKCS#8 PEM, readable by its owner alone
/// on Unix, which reads back as the same key; a file that exists is never replaced. Catches a key
/// file others can read, a format `openssl` does not write, and a second key over the first, which
/// would orphan every image the first signed.
#[test]
fn a_new_key_is_its_owners_alone_and_reads_back() {
    let folder = Folder::new("create");
    let path = folder.0.join("keys").join("signing-key.pem");
    let key = create_key(&path).expect("the key is made");
    let text = std::fs::read_to_string(&path).expect("the key file");
    assert!(
        text.starts_with("-----BEGIN PRIVATE KEY-----"),
        "{}",
        text.lines().next().unwrap_or("")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
    assert_eq!(
        read_key(&path).expect("the key reads").verifying_key(),
        key.verifying_key()
    );
    assert!(
        create_key(&path).is_err(),
        "an existing key is never replaced"
    );
    assert_eq!(
        read_key(&path)
            .expect("the first key stays")
            .verifying_key(),
        key.verifying_key()
    );
}

/// Specification 44 §5.11: a project that names its key, relative to its folder, signs with that
/// key, which must exist: a missing named key is an error naming its path, never a new key in the
/// project folder. Catches a key made where a shared project would carry it.
#[test]
fn a_project_signs_with_the_key_it_names() {
    let project = Folder::new("project");
    std::fs::write(
        project.0.join("trust-lsp.toml"),
        "[signing]\nkey = \"keys/site.pem\"\n",
    )
    .expect("the settings");
    let missing = project_key(&project.0).expect_err("a named key must exist");
    assert!(missing.to_string().contains("site.pem"), "{missing}");
    assert!(
        !project.0.join("keys").exists(),
        "no key is made in the project"
    );
    let key = create_key(&project.0.join("keys").join("site.pem")).expect("the site key");
    assert_eq!(
        project_key(&project.0)
            .expect("the named key")
            .verifying_key(),
        key.verifying_key()
    );
}

/// Specification 44 §5.11: a computer trusts the keys of `[signing] trusted_keys` in its
/// `device.toml`, each 64 hexadecimal digits; without the file it trusts none, and a key that is
/// not 32 bytes in hexadecimal is an error. Catches a malformed key skipped silently, which would
/// leave the device trusting fewer keys than its owner wrote, or none at all.
#[test]
fn a_computer_trusts_the_keys_of_its_device_file() {
    let folder = Folder::new("device");
    let path = folder.0.join("device.toml");
    assert_eq!(
        trusted_keys_in(&path).expect("no file"),
        Vec::<[u8; 32]>::new()
    );
    std::fs::write(
        &path,
        format!(
            "[signing]\ntrusted_keys = [\"{}\", \"{}\"]\n",
            "11".repeat(32),
            "AB".repeat(32)
        ),
    )
    .expect("the device file");
    assert_eq!(
        trusted_keys_in(&path).expect("two keys"),
        vec![[0x11; 32], [0xAB; 32]]
    );
    std::fs::write(&path, "[signing]\ntrusted_keys = [\"1234\"]\n").expect("the device file");
    let short = trusted_keys_in(&path).expect_err("a short key");
    assert!(short.to_string().contains("not 32 bytes"), "{short}");
}
