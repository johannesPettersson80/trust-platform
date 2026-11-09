//! CODESYS's system types under `__SYSTEM` (specification 06 §15): `__SYSTEM.ExceptionCode`, the
//! code a `__CATCH` variable takes, an enumeration with the explicit base `UDINT` and CODESYS's
//! values (CODESYS help, "Operator: __TRY, __CATCH, __FINALLY, __ENDTRY").

use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use super::{Symbol, SymbolId, SymbolKind, SymbolTable};
use crate::types::TypeId;

/// The name of the exception code type, as the symbol table registers it.
pub const EXCEPTION_CODE_TYPE: &str = "__SYSTEM.ExceptionCode";

/// The system type a `__SYSTEM.<name>` names, by its name after `__SYSTEM.`: `ExceptionCode`.
#[must_use]
pub fn system_type_name(name: &str) -> Option<&'static str> {
    name.eq_ignore_ascii_case("ExceptionCode")
        .then_some(EXCEPTION_CODE_TYPE)
}

/// CODESYS's exception codes, each value's name and its code.
pub const EXCEPTION_CODES: [(&str, u32); 49] = [
    ("RTSEXCPT_UNKNOWN", 0xFFFF_FFFF),
    ("RTSEXCPT_NOEXCEPTION", 0x0000_0000),
    ("RTSEXCPT_WATCHDOG", 0x0000_0010),
    ("RTSEXCPT_HARDWAREWATCHDOG", 0x0000_0011),
    ("RTSEXCPT_IO_CONFIG_ERROR", 0x0000_0012),
    ("RTSEXCPT_PROGRAMCHECKSUM", 0x0000_0013),
    ("RTSEXCPT_FIELDBUS_ERROR", 0x0000_0014),
    ("RTSEXCPT_IOUPDATE_ERROR", 0x0000_0015),
    ("RTSEXCPT_CYCLE_TIME_EXCEED", 0x0000_0016),
    ("RTSEXCPT_ONLCHANGE_PROGRAM_EXCEEDED", 0x0000_0017),
    ("RTSEXCPT_UNRESOLVED_EXTREFS", 0x0000_0018),
    ("RTSEXCPT_DOWNLOAD_REJECTED", 0x0000_0019),
    (
        "RTSEXCPT_BOOTPROJECT_REJECTED_DUE_RETAIN_ERROR",
        0x0000_001A,
    ),
    ("RTSEXCPT_LOADBOOTPROJECT_FAILED", 0x0000_001B),
    ("RTSEXCPT_OUT_OF_MEMORY", 0x0000_001C),
    ("RTSEXCPT_RETAIN_MEMORY_ERROR", 0x0000_001D),
    ("RTSEXCPT_BOOTPROJECT_CRASH", 0x0000_001E),
    ("RTSEXCPT_BOOTPROJECTTARGETMISMATCH", 0x0000_0021),
    ("RTSEXCPT_SCHEDULEERROR", 0x0000_0022),
    ("RTSEXCPT_FILE_CHECKSUM_ERR", 0x0000_0023),
    ("RTSEXCPT_RETAIN_IDENTITY_MISMATCH", 0x0000_0024),
    ("RTSEXCPT_IEC_TASK_CONFIG_ERROR", 0x0000_0025),
    ("RTSEXCPT_APP_TARGET_MISMATCH", 0x0000_0026),
    ("RTSEXCPT_ILLEGAL_INSTRUCTION", 0x0000_0050),
    ("RTSEXCPT_ACCESS_VIOLATION", 0x0000_0051),
    ("RTSEXCPT_PRIV_INSTRUCTION", 0x0000_0052),
    ("RTSEXCPT_IN_PAGE_ERROR", 0x0000_0053),
    ("RTSEXCPT_STACK_OVERFLOW", 0x0000_0054),
    ("RTSEXCPT_INVALID_DISPOSITION", 0x0000_0055),
    ("RTSEXCPT_INVALID_HANDLE", 0x0000_0056),
    ("RTSEXCPT_GUARD_PAGE", 0x0000_0057),
    ("RTSEXCPT_DOUBLE_FAULT", 0x0000_0058),
    ("RTSEXCPT_INVALID_OPCODE", 0x0000_0059),
    ("RTSEXCPT_MISALIGNMENT", 0x0000_0100),
    ("RTSEXCPT_ARRAYBOUNDS", 0x0000_0101),
    ("RTSEXCPT_DIVIDEBYZERO", 0x0000_0102),
    ("RTSEXCPT_OVERFLOW", 0x0000_0103),
    ("RTSEXCPT_NONCONTINUABLE", 0x0000_0104),
    ("RTSEXCPT_PROCESSORLOAD_WATCHDOG", 0x0000_0105),
    ("RTSEXCPT_FPU_ERROR", 0x0000_0150),
    ("RTSEXCPT_FPU_DENORMAL_OPERAND", 0x0000_0151),
    ("RTSEXCPT_FPU_DIVIDEBYZERO", 0x0000_0152),
    ("RTSEXCPT_FPU_INEXACT_RESULT", 0x0000_0153),
    ("RTSEXCPT_FPU_INVALID_OPERATION", 0x0000_0154),
    ("RTSEXCPT_FPU_OVERFLOW", 0x0000_0155),
    ("RTSEXCPT_FPU_STACK_CHECK", 0x0000_0156),
    ("RTSEXCPT_FPU_UNDERFLOW", 0x0000_0157),
    ("RTSEXCPT_VENDOR_EXCEPTION_BASE", 0x0000_2000),
    ("RTSEXCPT_USER_EXCEPTION_BASE", 0x0001_0000),
];

impl SymbolTable {
    /// Registers `__SYSTEM.ExceptionCode` and a symbol for each of its values. The values are in no
    /// scope: they are reached as `__SYSTEM.ExceptionCode.<value>`, as their type names them.
    pub(super) fn register_builtin_system_types(&mut self) {
        let values: Vec<(SmolStr, i64)> = EXCEPTION_CODES
            .iter()
            .map(|(name, code)| (SmolStr::new(name), i64::from(*code)))
            .collect();
        let type_id =
            self.register_named_value_type(EXCEPTION_CODE_TYPE, TypeId::UDINT, values.clone());
        let range = TextRange::empty(TextSize::from(0));
        for (name, value) in values {
            self.add_symbol_raw(Symbol::new(
                SymbolId::UNKNOWN,
                name,
                SymbolKind::EnumValue { value },
                type_id,
                range,
            ));
        }
    }
}
