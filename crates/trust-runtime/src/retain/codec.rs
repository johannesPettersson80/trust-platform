// The STRN v3 retain payload codec lives in trust-runtime-core, so a chip and a computer write the
// same bytes (specification 11 §6.7, specification 34 §10.1). The hosted `RetainSnapshot` wraps
// the core snapshot, so the codec is reached through the wrapper; the core owns every byte.

pub(crate) fn encode_snapshot(snapshot: &RetainSnapshot) -> Result<Vec<u8>, RuntimeError> {
    trust_runtime_core::retain::codec::encode_snapshot(&snapshot.inner)
}

pub(crate) fn decode_snapshot(bytes: &[u8]) -> Result<RetainSnapshot, RuntimeError> {
    trust_runtime_core::retain::codec::decode_snapshot(bytes)
        .map(|snapshot| RetainSnapshot::from_values(snapshot.into_values()))
}

// `store_tests` builds a legacy v1/v2 fixture by hand from the codec's wire format and value tags,
// so the hosted copy of the core codec's format constants is test-only.
#[cfg(test)]
const RETAIN_MAGIC: &[u8; 4] = b"STRN";
#[cfg(test)]
const RETAIN_V1_VERSION: u16 = 1;
#[cfg(test)]
const RETAIN_V2_VERSION: u16 = 2;
#[cfg(test)]
const RETAIN_VERSION: u16 = 3;
#[cfg(test)]
const RETAIN_TRAILER: &[u8; 4] = b"ENDR";

#[cfg(test)]
pub(crate) use trust_runtime_core::retain::codec::ValueTag;
