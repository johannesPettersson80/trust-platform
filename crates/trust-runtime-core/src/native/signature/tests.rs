//! A device's check of an image's signature (specification 44 §5.11, §10.2 step 4).

use alloc::vec;
use alloc::vec::Vec;

use ed25519_dalek::{Signer as _, SigningKey};

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

/// `bytes` signed by the key of `seed`, as a tool that deploys signs: the algorithm, the public
/// key and the signature over every byte before `SIGN`.
fn signed(mut bytes: Vec<u8>, seed: u8) -> (Vec<u8>, [u8; 32]) {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let start = Image::parse(&bytes)
        .expect("the image parses")
        .section(SectionId::Sign)
        .expect("a SIGN section")
        .offset as usize;
    let signature = key.sign(&bytes[..start]);
    bytes[start..start + 4].copy_from_slice(&ED25519.to_le_bytes());
    bytes[start + 4..start + 36].copy_from_slice(key.verifying_key().as_bytes());
    bytes[start + 36..start + SIGN_BYTES].copy_from_slice(&signature.to_bytes());
    (bytes, key.verifying_key().to_bytes())
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
/// unsigned. A `SIGN` of another length, or naming an algorithm other than 0 or 1, is malformed on
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
