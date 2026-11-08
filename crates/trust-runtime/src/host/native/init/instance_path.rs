//! The starting value of a variable marked `{attribute 'instance-path'}` (specification 10 §4.8): the
//! path of its instance, in its own module so that `init.rs` stays below the 1,000 lines of a file.

use smol_str::SmolStr;

use super::super::model::Model;
use super::super::types::{Kind, Ty};
use super::super::NativeError;
use super::{encode, Start};
use crate::value::Value;

impl Start {
    /// Writes the path of the instance `instance` into its variable `variable`, of the program's
    /// type `ty`, at `offset` of `bytes`, as an `instance-path` variable starts (specification 10
    /// §4.8): after the configuration's and the resource's names, cut to the variable's length;
    /// nothing for an instance a call creates, which has no place in the configuration.
    ///
    /// # Errors
    ///
    /// [`NativeError::Compile`] for a variable that is no `STRING`.
    pub(super) fn instance_path(
        &self,
        model: &mut Model<'_>,
        bytes: &mut [u8],
        offset: usize,
        ty: Ty,
        instance: &str,
        variable: &str,
    ) -> Result<(), NativeError> {
        let Some(prefix) = &self.prefix else {
            return Ok(());
        };
        if !matches!(model.types.info(ty).kind, Kind::Str(_)) {
            return Err(NativeError::Compile(format!(
                "{instance}.{variable} is marked instance-path, which takes a STRING (specification \
                 10 §4.8)"
            )));
        }
        let path = Value::String(SmolStr::new(format!("{prefix}{instance}")));
        encode(model, &path, ty, bytes, offset)
    }
}
