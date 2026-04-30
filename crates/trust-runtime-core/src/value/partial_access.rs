use super::{PartialAccess, PartialAccessError, Value};

pub fn read_partial_access(
    target: &Value,
    access: PartialAccess,
) -> Result<Value, PartialAccessError> {
    match (target, access) {
        (Value::Byte(value), PartialAccess::Bit(index)) => {
            if index > 7 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 7,
                });
            }
            Ok(Value::Bool(((value >> index) & 1) == 1))
        }
        (Value::Word(value), PartialAccess::Bit(index)) => {
            if index > 15 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 15,
                });
            }
            Ok(Value::Bool(((value >> index) & 1) == 1))
        }
        (Value::DWord(value), PartialAccess::Bit(index)) => {
            if index > 31 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 31,
                });
            }
            Ok(Value::Bool(((value >> index) & 1) == 1))
        }
        (Value::LWord(value), PartialAccess::Bit(index)) => {
            if index > 63 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 63,
                });
            }
            Ok(Value::Bool(((value >> index) & 1) == 1))
        }
        (Value::Word(value), PartialAccess::Byte(index)) => {
            if index > 1 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 1,
                });
            }
            let byte = ((value >> (index * 8)) & 0xFF) as u8;
            Ok(Value::Byte(byte))
        }
        (Value::DWord(value), PartialAccess::Byte(index)) => {
            if index > 3 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 3,
                });
            }
            let byte = ((value >> (index * 8)) & 0xFF) as u8;
            Ok(Value::Byte(byte))
        }
        (Value::LWord(value), PartialAccess::Byte(index)) => {
            if index > 7 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 7,
                });
            }
            let byte = ((value >> (index * 8)) & 0xFF) as u8;
            Ok(Value::Byte(byte))
        }
        (Value::DWord(value), PartialAccess::Word(index)) => {
            if index > 1 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 1,
                });
            }
            let word = ((value >> (index * 16)) & 0xFFFF) as u16;
            Ok(Value::Word(word))
        }
        (Value::LWord(value), PartialAccess::Word(index)) => {
            if index > 3 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 3,
                });
            }
            let word = ((value >> (index * 16)) & 0xFFFF) as u16;
            Ok(Value::Word(word))
        }
        (Value::LWord(value), PartialAccess::DWord(index)) => {
            if index > 1 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 1,
                });
            }
            let dword = ((value >> (index * 32)) & 0xFFFF_FFFF) as u32;
            Ok(Value::DWord(dword))
        }
        _ => Err(PartialAccessError::TypeMismatch),
    }
}

pub fn write_partial_access(
    target: Value,
    access: PartialAccess,
    value: Value,
) -> Result<Value, PartialAccessError> {
    match (target, access, value) {
        (Value::Byte(mut word), PartialAccess::Bit(index), Value::Bool(bit)) => {
            if index > 7 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 7,
                });
            }
            if bit {
                word |= 1 << index;
            } else {
                word &= !(1 << index);
            }
            Ok(Value::Byte(word))
        }
        (Value::Word(mut word), PartialAccess::Bit(index), Value::Bool(bit)) => {
            if index > 15 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 15,
                });
            }
            if bit {
                word |= 1 << index;
            } else {
                word &= !(1 << index);
            }
            Ok(Value::Word(word))
        }
        (Value::DWord(mut word), PartialAccess::Bit(index), Value::Bool(bit)) => {
            if index > 31 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 31,
                });
            }
            if bit {
                word |= 1 << index;
            } else {
                word &= !(1 << index);
            }
            Ok(Value::DWord(word))
        }
        (Value::LWord(mut word), PartialAccess::Bit(index), Value::Bool(bit)) => {
            if index > 63 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 63,
                });
            }
            if bit {
                word |= 1 << index;
            } else {
                word &= !(1 << index);
            }
            Ok(Value::LWord(word))
        }
        (Value::Word(mut word), PartialAccess::Byte(index), Value::Byte(byte)) => {
            if index > 1 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 1,
                });
            }
            let shift = index * 8;
            word &= !(0xFFu16 << shift);
            word |= (u16::from(byte)) << shift;
            Ok(Value::Word(word))
        }
        (Value::DWord(mut word), PartialAccess::Byte(index), Value::Byte(byte)) => {
            if index > 3 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 3,
                });
            }
            let shift = index * 8;
            word &= !(0xFFu32 << shift);
            word |= (u32::from(byte)) << shift;
            Ok(Value::DWord(word))
        }
        (Value::LWord(mut word), PartialAccess::Byte(index), Value::Byte(byte)) => {
            if index > 7 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 7,
                });
            }
            let shift = index * 8;
            word &= !(0xFFu64 << shift);
            word |= (u64::from(byte)) << shift;
            Ok(Value::LWord(word))
        }
        (Value::DWord(mut word), PartialAccess::Word(index), Value::Word(val)) => {
            if index > 1 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 1,
                });
            }
            let shift = index * 16;
            word &= !(0xFFFFu32 << shift);
            word |= (u32::from(val)) << shift;
            Ok(Value::DWord(word))
        }
        (Value::LWord(mut word), PartialAccess::Word(index), Value::Word(val)) => {
            if index > 3 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 3,
                });
            }
            let shift = index * 16;
            word &= !(0xFFFFu64 << shift);
            word |= (u64::from(val)) << shift;
            Ok(Value::LWord(word))
        }
        (Value::LWord(mut word), PartialAccess::DWord(index), Value::DWord(val)) => {
            if index > 1 {
                return Err(PartialAccessError::IndexOutOfBounds {
                    index: index as i64,
                    lower: 0,
                    upper: 1,
                });
            }
            let shift = index * 32;
            word &= !(0xFFFF_FFFFu64 << shift);
            word |= (u64::from(val)) << shift;
            Ok(Value::LWord(word))
        }
        _ => Err(PartialAccessError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::{read_partial_access, write_partial_access};
    use crate::value::{PartialAccess, PartialAccessError, Value};

    #[test]
    fn reads_partial_bits_and_words_with_bounds_errors() {
        assert_eq!(
            read_partial_access(&Value::Byte(0b0000_0010), PartialAccess::Bit(1)),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            read_partial_access(&Value::DWord(0xAABB_CCDD), PartialAccess::Word(1)),
            Ok(Value::Word(0xAABB))
        );
        assert_eq!(
            read_partial_access(&Value::Byte(0), PartialAccess::Bit(8)),
            Err(PartialAccessError::IndexOutOfBounds {
                index: 8,
                lower: 0,
                upper: 7,
            })
        );
    }

    #[test]
    fn writes_partial_bits_and_bytes_without_touching_other_bits() {
        assert_eq!(
            write_partial_access(Value::Byte(0), PartialAccess::Bit(2), Value::Bool(true)),
            Ok(Value::Byte(0b0000_0100))
        );
        assert_eq!(
            write_partial_access(
                Value::Word(0xABCD),
                PartialAccess::Byte(0),
                Value::Byte(0x12)
            ),
            Ok(Value::Word(0xAB12))
        );
        assert_eq!(
            write_partial_access(Value::Word(0), PartialAccess::DWord(0), Value::DWord(1)),
            Err(PartialAccessError::TypeMismatch)
        );
    }
}
