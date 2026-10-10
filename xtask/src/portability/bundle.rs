use anyhow::{ensure, Result};

pub(super) fn pack(artifacts: [&[u8]; 3]) -> Result<Vec<u8>> {
    let size = artifacts.iter().try_fold(32_usize, |sum, bytes| {
        ensure!(!bytes.is_empty(), "empty application artifact");
        sum.checked_add(bytes.len())
            .ok_or_else(|| anyhow::anyhow!("bundle size overflow"))
    })?;
    ensure!(
        size <= 16 * 1024,
        "application bundle {size} exceeds reserved sector"
    );
    let mut result = Vec::with_capacity(size);
    result.extend_from_slice(b"TRSTB002");
    for bytes in artifacts {
        result.extend_from_slice(&u32::try_from(bytes.len())?.to_le_bytes());
    }
    for bytes in artifacts {
        result.extend_from_slice(&crc32fast::hash(bytes).to_le_bytes());
    }
    for bytes in artifacts {
        result.extend_from_slice(bytes);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_bundle_preserves_three_artifacts_and_rejects_sector_overflow() {
        let bytes = pack([b"main", b"number", b"io"]).unwrap();
        assert_eq!(&bytes[..8], b"TRSTB002");
        assert_eq!(&bytes[8..20], &[4, 0, 0, 0, 6, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[32..], b"mainnumberio");
        assert_eq!(&bytes[28..32], &crc32fast::hash(b"io").to_le_bytes());
        assert!(pack([b"", b"b", b"c"]).is_err());
        assert!(pack([&vec![0; 16 * 1024], b"b", b"c"]).is_err());
    }
}
