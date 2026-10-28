//! Shared hardware-address overlap rule for declaration-based editor mappings.
use trust_runtime::io::{IoAddress, IoSize};

pub(super) fn overlaps(left: &IoAddress, right: &IoAddress) -> bool {
    if left.area != right.area {
        return false;
    }
    // Hierarchical points are distinct keys in the native process image.
    if left.path.len() > 1 || right.path.len() > 1 {
        return left.path == right.path && left.bit == right.bit && left.size == right.size;
    }
    fn bits(address: &IoAddress) -> (u64, u64) {
        let start = u64::from(address.byte) * 8
            + if address.size == IoSize::Bit {
                u64::from(address.bit)
            } else {
                0
            };
        let length = match address.size {
            IoSize::Bit => 1,
            IoSize::Byte => 8,
            IoSize::Word => 16,
            IoSize::DWord => 32,
            IoSize::LWord => 64,
            IoSize::Bytes(bytes) => u64::from(bytes) * 8,
        };
        (start, start + length)
    }
    let (a, b) = bits(left);
    let (c, d) = bits(right);
    a < d && c < b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_output_aliases_include_words_and_bits() {
        let parse = |text| IoAddress::parse(text).unwrap();
        assert!(overlaps(&parse("%QW0"), &parse("%QX1.7")));
        assert!(!overlaps(&parse("%QX0.0"), &parse("%QX0.1")));
        assert!(!overlaps(&parse("%QW0"), &parse("%QW2")));
        assert!(!overlaps(&parse("%IW0"), &parse("%QW0")));
        assert!(!overlaps(&parse("%QW1.2"), &parse("%QW1.3")));
    }
}
