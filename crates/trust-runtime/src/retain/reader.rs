// The retain byte reader lives in trust-runtime-core with the rest of the codec
// (specification 11 §6.7). `store_tests` builds a legacy v1/v2 fixture by hand and needs the
// codec's string framing.
#[cfg(test)]
pub(crate) use trust_runtime_core::retain::codec::encode_string;
