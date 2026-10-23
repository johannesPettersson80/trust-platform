use crate::error::RuntimeError;
use crate::value::SizeOfError;

pub(super) fn sizeof_error_to_runtime(err: SizeOfError) -> RuntimeError {
    match err {
        SizeOfError::Overflow => RuntimeError::Overflow,
        SizeOfError::UnknownType | SizeOfError::UnsupportedType => RuntimeError::TypeMismatch,
    }
}
