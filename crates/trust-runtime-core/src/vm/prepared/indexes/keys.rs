//! Compact, order-preserving keys for the two hot composite indexes.

/// Encodes `(owner: u8, pou: Option<u32>)` in the same lexicographic order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct DeclarationKey(u64);

impl DeclarationKey {
    pub(super) const fn new(owner: u8, pou: Option<u32>) -> Self {
        Self(((owner as u64) << 33) | encode_optional_pou(pou))
    }

    #[cfg(test)]
    pub(super) const fn parts(self) -> (u8, Option<u32>) {
        let owner = (self.0 >> 33) as u8;
        let encoded_pou = self.0 & ((1_u64 << 33) - 1);
        let pou = if encoded_pou == 0 {
            None
        } else {
            Some((encoded_pou - 1) as u32)
        };
        (owner, pou)
    }
}

/// Encodes `(pou: Option<u32>, phase: u8, trigger: u8)` in the same order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ActionKey(u64);

impl ActionKey {
    pub(super) const fn new(pou: Option<u32>, phase: u8, trigger: u8) -> Self {
        Self((encode_optional_pou(pou) << 16) | ((phase as u64) << 8) | trigger as u64)
    }

    pub(super) const fn parts(self) -> (Option<u32>, u8, u8) {
        let encoded_pou = (self.0 >> 16) & ((1_u64 << 33) - 1);
        let pou = if encoded_pou == 0 {
            None
        } else {
            Some((encoded_pou - 1) as u32)
        };
        let phase = ((self.0 >> 8) & 0xff) as u8;
        let trigger = (self.0 & 0xff) as u8;
        (pou, phase, trigger)
    }
}

/// None sorts before every POU ID; adding one preserves Some(0) and Some(u32::MAX).
const fn encode_optional_pou(pou: Option<u32>) -> u64 {
    match pou {
        None => 0,
        Some(pou) => pou as u64 + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POUS: [Option<u32>; 5] = [None, Some(0), Some(1), Some(u32::MAX - 1), Some(u32::MAX)];

    #[test]
    fn composite_index_keys_have_fixed_u64_storage() {
        assert_eq!(
            core::mem::size_of::<DeclarationKey>(),
            core::mem::size_of::<u64>()
        );
        assert_eq!(
            core::mem::size_of::<ActionKey>(),
            core::mem::size_of::<u64>()
        );
    }

    #[test]
    fn packed_declaration_keys_preserve_tuple_order_and_identity_at_boundaries() {
        for left_owner in [0, 1, u8::MAX] {
            for left_pou in POUS {
                for right_owner in [0, 1, u8::MAX] {
                    for right_pou in POUS {
                        let left_tuple = (left_owner, left_pou);
                        let right_tuple = (right_owner, right_pou);
                        let left = DeclarationKey::new(left_owner, left_pou);
                        let right = DeclarationKey::new(right_owner, right_pou);
                        assert_eq!(left.parts(), left_tuple);
                        assert_eq!(right.parts(), right_tuple);
                        assert_eq!(left.cmp(&right), left_tuple.cmp(&right_tuple));
                        assert_eq!(left == right, left_tuple == right_tuple);
                    }
                }
            }
        }
    }

    #[test]
    fn packed_action_keys_preserve_tuple_order_and_identity_at_boundaries() {
        for left_pou in POUS {
            for left_phase in [0, 1, u8::MAX] {
                for left_trigger in [0, 1, u8::MAX] {
                    for right_pou in POUS {
                        for right_phase in [0, 1, u8::MAX] {
                            for right_trigger in [0, 1, u8::MAX] {
                                let left_tuple = (left_pou, left_phase, left_trigger);
                                let right_tuple = (right_pou, right_phase, right_trigger);
                                let left = ActionKey::new(left_pou, left_phase, left_trigger);
                                let right = ActionKey::new(right_pou, right_phase, right_trigger);
                                assert_eq!(left.parts(), left_tuple);
                                assert_eq!(right.parts(), right_tuple);
                                assert_eq!(left.cmp(&right), left_tuple.cmp(&right_tuple));
                                assert_eq!(left == right, left_tuple == right_tuple);
                            }
                        }
                    }
                }
            }
        }
    }
}
