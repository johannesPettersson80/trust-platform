use super::defs::{Type, TypeId};
use super::registry::TypeRegistry;
use super::strings::DEFAULT_STRING_LENGTH;

impl TypeRegistry {
    pub(super) fn register_builtin_types(&mut self) {
        self.register_builtin(TypeId::UNKNOWN, "UNKNOWN", Type::Unknown);
        self.register_builtin(TypeId::VOID, "VOID", Type::Void);
        self.register_builtin(TypeId::NULL, "NULL", Type::Null);
        self.register_builtin(TypeId::BOOL, "BOOL", Type::Bool);
        self.register_builtin(TypeId::SINT, "SINT", Type::SInt);
        self.register_builtin(TypeId::INT, "INT", Type::Int);
        self.register_builtin(TypeId::DINT, "DINT", Type::DInt);
        self.register_builtin(TypeId::LINT, "LINT", Type::LInt);
        self.register_builtin(TypeId::USINT, "USINT", Type::USInt);
        self.register_builtin(TypeId::UINT, "UINT", Type::UInt);
        self.register_builtin(TypeId::UDINT, "UDINT", Type::UDInt);
        self.register_builtin(TypeId::ULINT, "ULINT", Type::ULInt);
        self.register_builtin(TypeId::REAL, "REAL", Type::Real);
        self.register_builtin(TypeId::LREAL, "LREAL", Type::LReal);
        self.register_builtin(TypeId::BYTE, "BYTE", Type::Byte);
        self.register_builtin(TypeId::WORD, "WORD", Type::Word);
        self.register_builtin(TypeId::DWORD, "DWORD", Type::DWord);
        self.register_builtin(TypeId::LWORD, "LWORD", Type::LWord);
        self.register_builtin(TypeId::TIME, "TIME", Type::Time);
        self.register_builtin(TypeId::LTIME, "LTIME", Type::LTime);
        self.register_builtin(TypeId::DATE, "DATE", Type::Date);
        self.register_builtin(TypeId::LDATE, "LDATE", Type::LDate);
        self.register_builtin(TypeId::TOD, "TOD", Type::Tod);
        self.register_builtin(TypeId::LTOD, "LTOD", Type::LTod);
        self.register_builtin(TypeId::DT, "DT", Type::Dt);
        self.register_builtin(TypeId::LDT, "LDT", Type::Ldt);
        // A STRING or WSTRING without a declared length is STRING[80] or WSTRING[80].
        self.register_builtin(
            TypeId::STRING,
            "STRING",
            Type::String {
                max_len: Some(DEFAULT_STRING_LENGTH),
            },
        );
        self.register_builtin(
            TypeId::WSTRING,
            "WSTRING",
            Type::WString {
                max_len: Some(DEFAULT_STRING_LENGTH),
            },
        );
        self.register_builtin(TypeId::CHAR, "CHAR", Type::Char);
        self.register_builtin(TypeId::WCHAR, "WCHAR", Type::WChar);
        self.register_builtin(TypeId::ANY, "ANY", Type::Any);
        self.register_builtin(TypeId::ANY_DERIVED, "ANY_DERIVED", Type::AnyDerived);
        self.register_builtin(
            TypeId::ANY_ELEMENTARY,
            "ANY_ELEMENTARY",
            Type::AnyElementary,
        );
        self.register_builtin(TypeId::ANY_MAGNITUDE, "ANY_MAGNITUDE", Type::AnyMagnitude);
        self.register_builtin(TypeId::ANY_INT, "ANY_INT", Type::AnyInt);
        self.register_builtin(TypeId::ANY_UNSIGNED, "ANY_UNSIGNED", Type::AnyUnsigned);
        self.register_builtin(TypeId::ANY_SIGNED, "ANY_SIGNED", Type::AnySigned);
        self.register_builtin(TypeId::ANY_REAL, "ANY_REAL", Type::AnyReal);
        self.register_builtin(TypeId::ANY_NUM, "ANY_NUM", Type::AnyNum);
        self.register_builtin(TypeId::ANY_DURATION, "ANY_DURATION", Type::AnyDuration);
        self.register_builtin(TypeId::ANY_BIT, "ANY_BIT", Type::AnyBit);
        self.register_builtin(TypeId::ANY_CHARS, "ANY_CHARS", Type::AnyChars);
        self.register_builtin(TypeId::ANY_STRING, "ANY_STRING", Type::AnyString);
        self.register_builtin(TypeId::ANY_CHAR, "ANY_CHAR", Type::AnyChar);
        self.register_builtin(TypeId::ANY_DATE, "ANY_DATE", Type::AnyDate);
    }
}
