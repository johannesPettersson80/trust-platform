//! Signing a program image (specification 44 §5.11): the project's key, or the user's own, which
//! the first signing creates readable by its owner alone and which no tool prints, fills the
//! image's `SIGN` section with the Ed25519 signature over every byte before it; and the keys this
//! computer trusts, from the `device.toml` of truST's configuration folder.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use ed25519_dalek::{Signer, SigningKey};
use rand::TryRng;
use serde::Deserialize;
use trust_runtime_core::native::image::{Image, SectionId};
use trust_runtime_core::native::signature::{ED25519, SIGN_BYTES};

use super::settings::signing_key_setting;
use super::NativeError;

/// The file of the user's own signing key in truST's configuration folder.
const USER_KEY_FILE: &str = "signing-key.pem";

/// The file of a computer's device settings in truST's configuration folder.
const DEVICE_FILE: &str = "device.toml";

/// truST's configuration folder of the user: `$XDG_CONFIG_HOME/trust`, else `~/.config/trust`, on
/// Linux; `~/Library/Application Support/trust` on macOS; `%APPDATA%\trust` on Windows.
#[must_use]
pub fn config_folder() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("trust"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/trust"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|folder| !folder.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|folder| folder.join("trust"))
    }
}

/// The signing key of the project in folder `root`: the key file `[signing] key` of its settings
/// names, which must exist, or else the user's own key, created on first use.
///
/// # Errors
///
/// [`NativeError::Signing`] for a key file that cannot be read or holds no Ed25519 PKCS#8 key, a
/// user without a configuration folder, or a key that cannot be created.
pub fn project_key(root: &Path) -> Result<SigningKey, NativeError> {
    match signing_key_setting(root)? {
        Some(path) => read_key(&path),
        None => user_key(),
    }
}

/// The user's own signing key, `signing-key.pem` in truST's configuration folder, which the first
/// call creates.
///
/// # Errors
///
/// As [`project_key`].
pub fn user_key() -> Result<SigningKey, NativeError> {
    let path = config_folder()
        .ok_or_else(|| {
            NativeError::Signing(String::from(
                "the user has no configuration folder for a signing key",
            ))
        })?
        .join(USER_KEY_FILE);
    if path.is_file() {
        return read_key(&path);
    }
    // Two builds may make the key at once: the one that links its file second reads the first's.
    create_key(&path).or_else(|error| {
        if path.is_file() {
            read_key(&path)
        } else {
            Err(error)
        }
    })
}

/// The Ed25519 key of the PKCS#8 PEM file at `path`.
///
/// # Errors
///
/// [`NativeError::Signing`] for a file that cannot be read or holds no such key.
pub fn read_key(path: &Path) -> Result<SigningKey, NativeError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| NativeError::Signing(format!("{}: {error}", path.display())))?;
    SigningKey::from_pkcs8_pem(&text).map_err(|_| {
        NativeError::Signing(format!(
            "{} holds no Ed25519 key in PKCS#8 PEM",
            path.display()
        ))
    })
}

/// A new Ed25519 key in a new PKCS#8 PEM file at `path`, readable by its owner alone; a file that
/// exists is never replaced. The key is written whole into a file of this process and linked to
/// `path` only then, so that no reader finds half a key.
///
/// # Errors
///
/// [`NativeError::Signing`] when the system gives no random bytes, the file cannot be written, or
/// `path` exists.
pub fn create_key(path: &Path) -> Result<SigningKey, NativeError> {
    let failed =
        |error: std::io::Error| NativeError::Signing(format!("{}: {error}", path.display()));
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    let mut seed = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut seed)
        .map_err(|error| NativeError::Signing(format!("no random bytes for a key: {error}")))?;
    let key = SigningKey::from_bytes(&seed);
    let pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|_| NativeError::Signing(String::from("the new key does not encode")))?;
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{}.new", std::process::id()));
    let written = PathBuf::from(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let linked = options
        .open(&written)
        .and_then(|mut file| std::io::Write::write_all(&mut file, pem.as_bytes()))
        .and_then(|()| std::fs::hard_link(&written, path));
    let _ = std::fs::remove_file(&written);
    linked.map_err(failed)?;
    Ok(key)
}

/// Signs `image` with `key` in place: its `SIGN` section takes the algorithm, the public key and
/// the signature over every byte before the section (specification 44 §5.11).
///
/// # Errors
///
/// [`NativeError::Image`] for bytes that are no image, or an image without a `SIGN` section of
/// its length.
pub fn sign(image: &mut [u8], key: &SigningKey) -> Result<(), NativeError> {
    let start = {
        let parsed = Image::parse(image).map_err(|error| NativeError::Image(error.to_string()))?;
        let section = parsed
            .section(SectionId::Sign)
            .filter(|section| usize::try_from(section.length) == Ok(SIGN_BYTES))
            .ok_or_else(|| NativeError::Image(String::from("the image has no SIGN section")))?;
        usize::try_from(section.offset)
            .map_err(|_| NativeError::Image(String::from("the SIGN section lies out of reach")))?
    };
    let signature = key.sign(&image[..start]);
    let content = &mut image[start..start + SIGN_BYTES];
    content[..4].copy_from_slice(&ED25519.to_le_bytes());
    content[4..36].copy_from_slice(key.verifying_key().as_bytes());
    content[36..].copy_from_slice(&signature.to_bytes());
    Ok(())
}

/// The part of `device.toml` a computer reads.
#[derive(Default, Deserialize)]
struct DeviceFile {
    #[serde(default)]
    signing: DeviceSigning,
}

#[derive(Default, Deserialize)]
struct DeviceSigning {
    #[serde(default)]
    trusted_keys: Vec<String>,
}

/// The keys `[signing] trusted_keys` of the device settings file at `path` names, each 64
/// hexadecimal digits; none without the file.
///
/// # Errors
///
/// [`NativeError::Signing`] for a file that cannot be read, is not TOML, or names a key that is
/// not 32 bytes in hexadecimal.
pub fn trusted_keys_in(path: &Path) -> Result<Vec<[u8; 32]>, NativeError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let invalid = |detail: String| NativeError::Signing(format!("{}: {detail}", path.display()));
    let text = std::fs::read_to_string(path).map_err(|error| invalid(error.to_string()))?;
    let file: DeviceFile = toml::from_str(&text).map_err(|error| invalid(error.to_string()))?;
    file.signing
        .trusted_keys
        .iter()
        .map(|text| {
            let bytes = hex::decode(text.trim())
                .map_err(|_| invalid(format!("the trusted key '{text}' is not hexadecimal")))?;
            <[u8; 32]>::try_from(bytes.as_slice())
                .map_err(|_| invalid(format!("the trusted key '{text}' is not 32 bytes")))
        })
        .collect()
}

/// The keys this computer trusts (specification 44 §5.11): those of the `device.toml` in truST's
/// configuration folder of the user that runs trust-runtime, read once; none without the file.
///
/// # Errors
///
/// [`NativeError::Signing`] for a file that cannot be read or a key it names that is not one, every
/// time it is asked, so that a computer whose trust cannot be read admits no image.
pub fn computer_trusted_keys() -> Result<&'static [[u8; 32]], NativeError> {
    static TRUSTED: OnceLock<Result<&'static [[u8; 32]], String>> = OnceLock::new();
    TRUSTED
        .get_or_init(|| {
            let keys = match config_folder() {
                Some(folder) => trusted_keys_in(&folder.join(DEVICE_FILE)),
                None => Ok(Vec::new()),
            };
            keys.map(|keys| &*Box::leak(keys.into_boxed_slice()))
                .map_err(|error| error.to_string())
        })
        .clone()
        .map_err(NativeError::Signing)
}

#[cfg(test)]
#[path = "signing/tests.rs"]
mod tests;
