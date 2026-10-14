use super::{BytecodeError, BytecodeReader};
use alloc::vec;

type ReadBits = for<'a> fn(&mut BytecodeReader<'a>) -> Result<u64, BytecodeError>;

#[test]
fn byte_ranges_reject_oversized_lengths_without_advancing_the_cursor() {
    let bytes = [0x19, 0x53, 0x81];
    for prefix in 0..=bytes.len() {
        let mut reader = BytecodeReader::new(&bytes);
        assert_eq!(reader.read_bytes(prefix), Ok(&bytes[..prefix]));
        for length in [bytes.len() + 1, usize::MAX - 1, usize::MAX] {
            assert_eq!(reader.read_bytes(length), Err(BytecodeError::UnexpectedEof));
            assert_eq!(reader.pos(), prefix);
            assert_eq!(reader.remaining(), bytes.len() - prefix);
        }
        assert_eq!(reader.read_bytes(0), Ok(&bytes[prefix..prefix]));
        assert_eq!(
            reader.read_bytes(bytes.len() - prefix),
            Ok(&bytes[prefix..])
        );
        assert_eq!(reader.read_bytes(0), Ok(&bytes[bytes.len()..]));
        assert_eq!(reader.pos(), bytes.len());
    }
}

#[test]
fn fixed_width_readers_preserve_bits_and_every_eof_cursor_boundary() {
    let readers: [(usize, ReadBits); 6] = [
        (1, |r| r.read_u8().map(u64::from)),
        (2, |r| r.read_u16().map(u64::from)),
        (4, |r| r.read_u32().map(u64::from)),
        (8, |r| r.read_u64()),
        (4, |r| r.read_i32().map(|v| u64::from(v as u32))),
        (8, |r| r.read_i64().map(|v| v as u64)),
    ];
    for (width, read) in readers {
        for byte in [0, 0x7f, 0x80, 0xff] {
            let payload = vec![byte; width];
            let expected = payload
                .iter()
                .enumerate()
                .fold(0u64, |bits, (offset, value)| {
                    bits | (u64::from(*value) << (8 * offset))
                });
            for length in 0..=width {
                let mut bytes = vec![0x19, 0x53];
                bytes.extend_from_slice(&payload[..length]);
                let mut reader = BytecodeReader::new(&bytes);
                assert_eq!(reader.read_u16(), Ok(0x5319));
                let result = read(&mut reader);
                if length == width {
                    assert_eq!(result, Ok(expected));
                    assert_eq!(reader.pos(), 2 + width);
                    assert_eq!(reader.remaining(), 0);
                    assert_eq!(reader.read_u8(), Err(BytecodeError::UnexpectedEof));
                    assert_eq!(reader.pos(), 2 + width);
                } else {
                    assert_eq!(result, Err(BytecodeError::UnexpectedEof));
                    assert_eq!(reader.pos(), 2);
                    assert_eq!(reader.remaining(), length);
                    assert_eq!(reader.read_bytes(length), Ok(&payload[..length]));
                }
            }
        }
    }
}

#[test]
fn fixed_width_readers_keep_mixed_signed_little_endian_values() {
    let bytes = [0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0xff];
    assert_eq!(
        BytecodeReader::new(&bytes).read_u64(),
        Ok(0xffed_cba9_8765_4321)
    );
    assert_eq!(
        BytecodeReader::new(&bytes).read_i64(),
        Ok(0xffed_cba9_8765_4321u64 as i64)
    );
    assert_eq!(BytecodeReader::new(&bytes).read_u32(), Ok(0x8765_4321));
    assert_eq!(
        BytecodeReader::new(&bytes).read_i32(),
        Ok(0x8765_4321u32 as i32)
    );
    assert_eq!(BytecodeReader::new(&bytes).read_u16(), Ok(0x4321));
    assert_eq!(BytecodeReader::new(&bytes).read_u8(), Ok(0x21));
}
