use alloc::vec::Vec;

use super::*;

/// The entry of sequence 2 in the state `New`, byte for byte as ESP-IDF writes it (the test data of
/// esp-bootloader-esp-idf 0.6.0's `ota.rs`, `SLOT_COUNT_2_NEW`).
const SEQUENCE_2_NEW: [u8; 32] = [
    2, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 0, 0, 0, 0, 116, 55, 246, 85,
];

fn entry(sequence: u32, state: ImageState) -> Option<SelectEntry> {
    Some(SelectEntry { sequence, state })
}

/// The CRC of a sequence is the ROM's `crc32_le` from `UINT32_MAX`: 0x4743989A for 1 and 0x55F63774
/// for 2 (ESP-IDF's own entries). Catches the plain CRC-32 of the four bytes, 0x99F8B879 for 1, which
/// the boot loader would read as a damaged entry and so boot the other slot.
#[test]
fn a_sequence_crc_is_the_boot_loaders() {
    assert_eq!(sequence_crc(1), 0x4743_989A);
    assert_eq!(sequence_crc(2), 0x55F6_3774);
}

/// An entry is written as ESP-IDF writes it and read back; an erased entry and one whose CRC is wrong
/// are no entry. Catches a label or state at another offset, and a damaged entry read as valid.
#[test]
fn entries_are_written_and_read_as_esp_idf_does() {
    let written = encode_entry(SelectEntry {
        sequence: 2,
        state: ImageState::New,
    });
    assert_eq!(written, SEQUENCE_2_NEW);
    assert_eq!(decode_entry(&written), entry(2, ImageState::New));
    assert_eq!(decode_entry(&[0xFF; 32]), None);
    let mut damaged = SEQUENCE_2_NEW;
    damaged[0] = 3;
    assert_eq!(decode_entry(&damaged), None);
}

/// The boot loader boots the bootable entry with the highest sequence, slot (sequence - 1) % 2, and
/// slot A without one; an `Aborted` or `Invalid` entry is not bootable. Worked by hand: sequence 1
/// names A and 2 names B. Catches a selection by entry index instead of sequence, and a firmware the
/// boot loader stopped booting taken for the running one.
#[test]
fn the_boot_loader_boots_the_newest_bootable_entry() {
    assert_eq!(selected_slot(&[None, None]), Slot::A);
    assert_eq!(selected_slot(&[entry(1, ImageState::Valid), None]), Slot::A);
    assert_eq!(
        selected_slot(&[entry(1, ImageState::Valid), entry(2, ImageState::New)]),
        Slot::B
    );
    assert_eq!(
        selected_slot(&[
            entry(3, ImageState::PendingVerify),
            entry(2, ImageState::Valid)
        ]),
        Slot::A
    );
    assert_eq!(
        selected_slot(&[entry(1, ImageState::Valid), entry(2, ImageState::Aborted)]),
        Slot::A
    );
    assert_eq!(
        selected_slot(&[entry(1, ImageState::Valid), entry(2, ImageState::Invalid)]),
        Slot::A
    );
}

/// An update is selected in the entry that is not active, in the state `New`, with the first sequence
/// from the active one's that names the target, as `esp_ota_set_boot_partition` writes it. Worked by
/// hand: no active entry and target B gives entry 0 with sequence 2; A at 1 to B gives entry 1 at 2; B
/// at 2 to A gives entry 0 at 3; A at 3 with an aborted B at 4 to B gives entry 1 at 4. Catches
/// overwriting the active entry, which leaves no firmware to roll back to, and a sequence that names
/// the running slot again.
#[test]
fn an_update_is_selected_in_the_other_entry_on_trial() {
    let new = |sequence| SelectEntry {
        sequence,
        state: ImageState::New,
    };
    assert_eq!(activation(&[None, None], Slot::B), (0, new(2)));
    assert_eq!(
        activation(&[entry(1, ImageState::Valid), None], Slot::B),
        (1, new(2))
    );
    assert_eq!(
        activation(
            &[entry(1, ImageState::Valid), entry(2, ImageState::Valid)],
            Slot::A
        ),
        (0, new(3))
    );
    assert_eq!(
        activation(
            &[entry(3, ImageState::Valid), entry(4, ImageState::Aborted)],
            Slot::B
        ),
        (1, new(4))
    );
    for (entries, target) in [
        ([entry(1, ImageState::Valid), None], Slot::B),
        (
            [entry(1, ImageState::Valid), entry(2, ImageState::Valid)],
            Slot::A,
        ),
    ] {
        let (index, written) = activation(&entries, target);
        let mut after = entries;
        after[index] = Some(written);
        assert_eq!(selected_slot(&after), target, "{entries:?}");
    }
}

/// At boot a firmware reads whether it runs on trial and whether the boot loader rolled a newer
/// firmware back. Catches a trial firmware that never confirms itself, which the next reset rolls
/// back, and a rollback that is not reported.
#[test]
fn the_boot_reads_the_trial_and_a_rollback() {
    let trial = boot_selection(
        &[
            entry(1, ImageState::Valid),
            entry(2, ImageState::PendingVerify),
        ],
        Slot::B,
    );
    assert_eq!(
        trial,
        BootSelection {
            running: Slot::B,
            entry: Some(1),
            trial: true,
            rolled_back: false,
        }
    );
    let rolled_back = boot_selection(
        &[entry(1, ImageState::Valid), entry(2, ImageState::Aborted)],
        Slot::A,
    );
    assert_eq!(
        rolled_back,
        BootSelection {
            running: Slot::A,
            entry: Some(0),
            trial: false,
            rolled_back: true,
        }
    );
    let first = boot_selection(&[None, None], Slot::A);
    assert_eq!(
        (first.entry, first.trial, first.rolled_back),
        (None, false, false)
    );
}

/// A firmware image as `espflash save-image` writes it: the header, one segment that begins with the
/// application descriptor, the checksum on the next 16-byte boundary and the SHA-256.
pub(crate) fn test_image(chip: u16, project: &str, version: &str) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0xABCD_5432u32.to_le_bytes());
    data.extend_from_slice(&[0; 12]);
    let mut field = [0u8; 32];
    field[..version.len()].copy_from_slice(version.as_bytes());
    data.extend_from_slice(&field);
    let mut field = [0u8; 32];
    field[..project.len()].copy_from_slice(project.as_bytes());
    data.extend_from_slice(&field);
    data.extend_from_slice(&[0x5A; 100]);
    let mut image = Vec::new();
    let mut header = [0u8; 24];
    header[0] = 0xE9;
    header[1] = 1;
    header[12..14].copy_from_slice(&chip.to_le_bytes());
    header[23] = 1;
    image.extend_from_slice(&header);
    image.extend_from_slice(&0x4200_0020u32.to_le_bytes());
    image.extend_from_slice(
        &u32::try_from(data.len())
            .expect("a short segment")
            .to_le_bytes(),
    );
    image.extend_from_slice(&data);
    let checksum = data.iter().fold(0xEFu8, |sum, byte| sum ^ byte);
    while image.len() % 16 != 15 {
        image.push(0);
    }
    image.push(checksum);
    let digest = Sha256::digest(&image);
    image.extend_from_slice(&digest);
    image
}

fn check(bytes: &[u8]) -> Result<AppImage, ImageError> {
    let len = u32::try_from(bytes.len()).unwrap();
    check_image(len, &mut |at, buffer| {
        let start = usize::try_from(at).map_err(|_| ())?;
        let end = start + buffer.len();
        buffer.copy_from_slice(bytes.get(start..end).ok_or(())?);
        Ok(())
    })
}

/// truST's firmware image passes with its descriptor's version; 24 + 8 + 180 bytes of segment end at
/// 212, so the checksum is byte 223 and the image 256 bytes with its SHA-256. Catches an image of
/// another chip or project, a damaged segment byte, digest or length, and an upload with more or
/// fewer bytes than the image, each installed.
#[test]
fn a_firmware_image_is_checked_before_it_is_selected() {
    let good = test_image(13, PROJECT, "0.24.76");
    assert_eq!(good.len(), 256);
    assert_eq!(
        check(&good),
        Ok(AppImage {
            version: "0.24.76".into(),
            bytes: 224,
        })
    );
    let mut magic = good.clone();
    magic[0] = 0xE8;
    assert_eq!(check(&magic), Err(ImageError::Magic));
    assert_eq!(
        check(&test_image(5, PROJECT, "0.24.76")),
        Err(ImageError::Chip(5))
    );
    assert_eq!(
        check(&test_image(13, "hello_world", "1.0")),
        Err(ImageError::Project("hello_world".into()))
    );
    let mut segment = good.clone();
    segment[150] ^= 1;
    assert_eq!(check(&segment), Err(ImageError::Checksum));
    let mut digest = good.clone();
    digest[255] ^= 1;
    assert_eq!(check(&digest), Err(ImageError::Digest));
    let mut longer = good.clone();
    longer.push(0xFF);
    assert_eq!(
        check(&longer),
        Err(ImageError::Trailing {
            image: 256,
            upload: 257,
        })
    );
    assert_eq!(check(&good[..240]), Err(ImageError::Truncated));
}

/// The slots lie where the partition table puts `ota_0` and `ota_1`; an address between or after
/// them is in no slot. Catches a slot boundary off by one erase unit.
#[test]
fn the_slots_are_ota_0_and_ota_1() {
    assert_eq!(slot_at(0x1_0000), Some(Slot::A));
    assert_eq!(slot_at(0x1F_FFFF), Some(Slot::A));
    assert_eq!(slot_at(0x20_0000), Some(Slot::B));
    assert_eq!(slot_at(0x3E_FFFF), Some(Slot::B));
    assert_eq!(slot_at(0x3F_0000), None);
    assert_eq!(slot_at(0xF000), None);
}
