//! A device's check of an image's signature (specification 44 §5.11, §10.2 step 4).

use alloc::vec;
use alloc::vec::Vec;

use ed25519_dalek::{Signer as _, SigningKey};
use sha2::Sha512;

use super::*;
use crate::native::image::{Image, SectionId, Target};
use crate::native::program::tests::{output_bit, writer_with};

/// The test image of the program tests with a `SIGN` section of `bytes` zeros.
fn with_sign(bytes: usize) -> Vec<u8> {
    let mut writer = writer_with(0, 100, 50, Target::X86_64, output_bit(), 0, 0);
    writer.section(SectionId::Sign, vec![0; bytes]);
    writer.finish().expect("the image writes")
}

/// The test image with `SIGN` reserved, unsigned.
fn reserved() -> Vec<u8> {
    with_sign(SIGN_BYTES)
}

/// Where `SIGN` starts in `bytes`: the end of the bytes a signature covers.
fn sign_start(bytes: &[u8]) -> usize {
    Image::parse(bytes)
        .expect("the image parses")
        .section(SectionId::Sign)
        .expect("a SIGN section")
        .offset as usize
}

/// `bytes` with `SIGN` holding `algorithm`, the public key of `key` and `signature`.
fn with_claim(
    mut bytes: Vec<u8>,
    algorithm: u32,
    key: &SigningKey,
    signature: [u8; 64],
) -> Vec<u8> {
    let start = sign_start(&bytes);
    bytes[start..start + 4].copy_from_slice(&algorithm.to_le_bytes());
    bytes[start + 4..start + 36].copy_from_slice(key.verifying_key().as_bytes());
    bytes[start + 36..start + SIGN_BYTES].copy_from_slice(&signature);
    bytes
}

/// `bytes` signed by the key of `seed` with `context`: Ed25519ph over the SHA-512 of every byte
/// before `SIGN`.
fn signed_with(bytes: Vec<u8>, seed: u8, context: &[u8]) -> (Vec<u8>, [u8; 32]) {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let start = sign_start(&bytes);
    let signature = key
        .sign_prehashed(Sha512::new_with_prefix(&bytes[..start]), Some(context))
        .expect("the context fits");
    let public = key.verifying_key().to_bytes();
    (
        with_claim(bytes, ED25519PH, &key, signature.to_bytes()),
        public,
    )
}

/// `bytes` signed by the key of `seed`, as a tool that deploys signs (specification 44 §5.11).
fn signed(bytes: Vec<u8>, seed: u8) -> (Vec<u8>, [u8; 32]) {
    signed_with(bytes, seed, CONTEXT)
}

/// The public key of the key of `seed`.
fn public(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}

fn check_bytes(bytes: &[u8], trusted: &[[u8; 32]]) -> Result<(), SignatureError> {
    check(&Image::parse(bytes).expect("the image parses"), trusted)
}

/// Specification 44 §5.11: a device that trusts no key admits an image without `SIGN`, one whose
/// `SIGN` is reserved and unsigned, and one signed by any key; a device that trusts a key admits
/// only an image that key signed, and refuses an unsigned one and one another key signed. Catches a
/// device with a list that admits an unsigned image or a stranger's, and one without a list that
/// refuses an unsigned image, which development needs.
#[test]
fn a_device_admits_what_its_trusted_keys_allow() {
    let without = crate::native::program::tests::image(0, 100, 50, Target::X86_64);
    assert_eq!(check_bytes(&without, &[]), Ok(()));
    assert_eq!(check_bytes(&reserved(), &[]), Ok(()));
    let (image, key) = signed(reserved(), 7);
    assert_eq!(check_bytes(&image, &[]), Ok(()));
    assert_eq!(check_bytes(&image, &[key]), Ok(()));
    assert_eq!(check_bytes(&image, &[public(8), key]), Ok(()));
    assert_eq!(
        check_bytes(&image, &[public(8)]),
        Err(SignatureError::Untrusted)
    );
    assert_eq!(
        check_bytes(&reserved(), &[key]),
        Err(SignatureError::Untrusted)
    );
    assert_eq!(
        check_bytes(&without, &[key]),
        Err(SignatureError::Untrusted)
    );
}

/// Specification 44 §5.11: a byte changed after signing, here one of the module's code, which the
/// header's CRC does not cover, fails the signature on a device that trusts the signer; a device
/// that trusts another key refuses the signer before it verifies; a device that trusts no key
/// checks no signature, so it admits the image, as it would admit the same image re-signed or
/// unsigned. A `SIGN` of another length, or naming an algorithm other than 0 or 2, is malformed on
/// every device. Catches a signature that covers less than every byte before `SIGN`, an unknown
/// algorithm taken as unsigned, and a device without a list that spends a signature check (0.85 s
/// for a 90 KB image on the ESP32-C6) that proves nothing the record's hash does not.
#[test]
fn a_changed_byte_or_a_malformed_section_is_refused() {
    let (mut image, key) = signed(reserved(), 7);
    let code = Image::parse(&image)
        .expect("the image parses")
        .section(SectionId::Code)
        .expect("CODE")
        .offset as usize;
    image[code] ^= 0x01;
    assert_eq!(check_bytes(&image, &[key]), Err(SignatureError::Invalid));
    assert_eq!(
        check_bytes(&image, &[public(8)]),
        Err(SignatureError::Untrusted)
    );
    assert_eq!(check_bytes(&image, &[]), Ok(()));

    let mut unknown = reserved();
    let start = Image::parse(&unknown)
        .expect("the image parses")
        .section(SectionId::Sign)
        .expect("SIGN")
        .offset as usize;
    unknown[start] = 7;
    assert_eq!(check_bytes(&unknown, &[]), Err(SignatureError::Malformed));
    assert_eq!(
        check_bytes(&unknown, &[key]),
        Err(SignatureError::Malformed)
    );
    assert_eq!(
        check_bytes(&with_sign(SIGN_BYTES - 1), &[]),
        Err(SignatureError::Malformed)
    );
}

/// RFC 8032 §7.3, the Ed25519ph test "abc": its secret key gives its public key, and signs the
/// SHA-512 of `abc` with the empty context to its signature, byte for byte (computed again from the
/// RFC's definitions in Python before it was pinned here). The kernel's check uses this primitive with
/// the context `truST program image` (19 bytes), so an image signed with the empty context, or
/// signed in the pure Ed25519 of 0.20 under algorithm 1, is refused. Catches a check that verifies
/// another scheme than RFC 8032's Ed25519ph, a context left out, and the old algorithm still taken.
#[test]
fn the_signature_is_rfc_8032_ed25519ph_with_the_image_context() {
    let decode = |text: &str| -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
            .collect()
    };
    let secret: [u8; 32] =
        decode("833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42")
            .try_into()
            .expect("32 bytes");
    let key = SigningKey::from_bytes(&secret);
    assert_eq!(
        key.verifying_key().to_bytes().to_vec(),
        decode("ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf")
    );
    let signature = key
        .sign_prehashed(Sha512::new_with_prefix(b"abc"), None)
        .expect("signs");
    assert_eq!(
        signature.to_bytes().to_vec(),
        decode(concat!(
            "98a70222f0b8121aa9d30f813d683f809e462b469c7ff87639499bb94e6dae41",
            "31f85042463c2a355a2003d062adf5aaa10b8c61e636062aaad11c2a26083406"
        ))
    );
    assert_eq!(CONTEXT, b"truST program image");
    assert_eq!(CONTEXT.len(), 19);

    let (empty_context, key_7) = signed_with(reserved(), 7, b"");
    assert_eq!(
        check_bytes(&empty_context, &[key_7]),
        Err(SignatureError::Invalid)
    );
    let pure_key = SigningKey::from_bytes(&[7; 32]);
    let bytes = reserved();
    let pure = pure_key.sign(&bytes[..sign_start(&bytes)]).to_bytes();
    let pure_image = with_claim(bytes, 1, &pure_key, pure);
    assert_eq!(
        check_bytes(&pure_image, &[key_7]),
        Err(SignatureError::Malformed)
    );
    let (image, _) = signed(reserved(), 7);
    assert_eq!(check_bytes(&image, &[key_7]), Ok(()));
}

/// A prehash that a device builds part by part, here from three uneven parts of the bytes before
/// `SIGN`, admits the image as hashing the whole does; a prehash that misses the last byte, or that
/// hashed one byte changed, fails the signature; a device that trusts no key admits the image
/// whatever the prehash. Catches a check that ignores the prehash it is given (it would spend the
/// hashing again), and one that takes a prehash of other bytes.
#[test]
fn a_prehash_built_part_by_part_admits_as_hashing_the_whole_does() {
    let (image, key) = signed(reserved(), 7);
    let start = sign_start(&image);
    let parsed = Image::parse(&image).expect("the image parses");
    let mut parts = Prehash::new();
    parts.update(&image[..1]);
    parts.update(&image[1..start / 2]);
    parts.update(&image[start / 2..start]);
    assert_eq!(check_prehashed(&parsed, &[key], parts), Ok(()));

    let mut short = Prehash::new();
    short.update(&image[..start - 1]);
    assert_eq!(
        check_prehashed(&parsed, &[key], short.clone()),
        Err(SignatureError::Invalid)
    );
    let mut changed = image[..start].to_vec();
    changed[start / 2] ^= 0x01;
    let mut other = Prehash::new();
    other.update(&changed);
    assert_eq!(
        check_prehashed(&parsed, &[key], other),
        Err(SignatureError::Invalid)
    );
    assert_eq!(check_prehashed(&parsed, &[], short), Ok(()));
}
