use sha2::{Digest, Sha256};

/// Hashes a backend catalog snapshot without depending on row-return order.
pub(super) fn fingerprint(mut catalog_rows: Vec<String>) -> String {
    catalog_rows.sort_unstable();
    let mut digest = Sha256::new();
    for row in catalog_rows {
        digest.update(row.len().to_be_bytes());
        digest.update(row.as_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn incompatible(backend: &str) -> super::PersistenceError {
    super::PersistenceError::Commit(format!(
        "{backend} incompatible pre-release schema: the generation-1 catalog definition changed; back up and recreate the development database"
    ))
}
