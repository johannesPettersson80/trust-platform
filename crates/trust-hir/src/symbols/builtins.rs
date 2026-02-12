use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use crate::types::{Type, TypeId};

use super::defs::{ParamDirection, Symbol, SymbolId, SymbolKind};
use super::table::SymbolTable;

impl SymbolTable {
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
        self.register_builtin(TypeId::TOD, "TIME_OF_DAY", Type::Tod);
        self.register_builtin(TypeId::LTOD, "LTIME_OF_DAY", Type::LTod);
        self.register_builtin(TypeId::DT, "DATE_AND_TIME", Type::Dt);
        self.register_builtin(TypeId::LDT, "LDATE_AND_TIME", Type::Ldt);
        self.register_builtin(TypeId::STRING, "STRING", Type::String { max_len: None });
        self.register_builtin(TypeId::WSTRING, "WSTRING", Type::WString { max_len: None });
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

    pub(super) fn register_builtin_function_blocks(&mut self) {
        self.register_bistable_function_blocks();
        self.register_edge_detection_function_blocks();
        self.register_counter_function_blocks();

        self.register_timer_function_block("TP", TypeId::TIME);
        self.register_timer_function_block("TON", TypeId::TIME);
        self.register_timer_function_block("TOF", TypeId::TIME);

        self.register_timer_function_block("TP_LTIME", TypeId::LTIME);
        self.register_timer_function_block("TON_LTIME", TypeId::LTIME);
        self.register_timer_function_block("TOF_LTIME", TypeId::LTIME);
    }

    fn register_bistable_function_blocks(&mut self) {
        self.register_simple_function_block(
            "RS",
            &[
                ("S", TypeId::BOOL, ParamDirection::In),
                ("R1", TypeId::BOOL, ParamDirection::In),
                ("Q1", TypeId::BOOL, ParamDirection::Out),
            ],
        );
        self.register_simple_function_block(
            "SR",
            &[
                ("S1", TypeId::BOOL, ParamDirection::In),
                ("R", TypeId::BOOL, ParamDirection::In),
                ("Q1", TypeId::BOOL, ParamDirection::Out),
            ],
        );
    }

    fn register_edge_detection_function_blocks(&mut self) {
        self.register_simple_function_block(
            "R_TRIG",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        );
        self.register_simple_function_block(
            "F_TRIG",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        );
        self.register_simple_function_block(
            "DIFU",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        );
        self.register_simple_function_block(
            "DIFD",
            &[
                ("CLK", TypeId::BOOL, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
            ],
        );
    }

    fn register_counter_function_blocks(&mut self) {
        self.register_counter_up_function_block("CTU", TypeId::ANY_INT);
        self.register_counter_down_function_block("CTD", TypeId::ANY_INT);
        self.register_counter_up_down_function_block("CTUD", TypeId::ANY_INT);

        let variants = [
            ("INT", TypeId::INT),
            ("DINT", TypeId::DINT),
            ("LINT", TypeId::LINT),
            ("UDINT", TypeId::UDINT),
            ("ULINT", TypeId::ULINT),
        ];

        for (suffix, type_id) in variants {
            self.register_counter_up_function_block(&format!("CTU_{}", suffix), type_id);
            self.register_counter_down_function_block(&format!("CTD_{}", suffix), type_id);
            self.register_counter_up_down_function_block(&format!("CTUD_{}", suffix), type_id);
        }
    }

    fn register_counter_up_function_block(&mut self, name: &str, value_type: TypeId) {
        self.register_simple_function_block(
            name,
            &[
                ("CU", TypeId::BOOL, ParamDirection::In),
                ("R", TypeId::BOOL, ParamDirection::In),
                ("PV", value_type, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
                ("CV", value_type, ParamDirection::Out),
            ],
        );
    }

    fn register_counter_down_function_block(&mut self, name: &str, value_type: TypeId) {
        self.register_simple_function_block(
            name,
            &[
                ("CD", TypeId::BOOL, ParamDirection::In),
                ("LD", TypeId::BOOL, ParamDirection::In),
                ("PV", value_type, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
                ("CV", value_type, ParamDirection::Out),
            ],
        );
    }

    fn register_counter_up_down_function_block(&mut self, name: &str, value_type: TypeId) {
        self.register_simple_function_block(
            name,
            &[
                ("CU", TypeId::BOOL, ParamDirection::In),
                ("CD", TypeId::BOOL, ParamDirection::In),
                ("R", TypeId::BOOL, ParamDirection::In),
                ("LD", TypeId::BOOL, ParamDirection::In),
                ("PV", value_type, ParamDirection::In),
                ("QU", TypeId::BOOL, ParamDirection::Out),
                ("QD", TypeId::BOOL, ParamDirection::Out),
                ("CV", value_type, ParamDirection::Out),
            ],
        );
    }

    fn register_timer_function_block(&mut self, name: &str, time_type: TypeId) {
        self.register_simple_function_block(
            name,
            &[
                ("IN", TypeId::BOOL, ParamDirection::In),
                ("PT", time_type, ParamDirection::In),
                ("Q", TypeId::BOOL, ParamDirection::Out),
                ("ET", time_type, ParamDirection::Out),
            ],
        );
    }

    fn register_simple_function_block(
        &mut self,
        name: &str,
        params: &[(&str, TypeId, ParamDirection)],
    ) {
        let name = SmolStr::new(name);
        let type_id = self.register_type(name.clone(), Type::FunctionBlock { name: name.clone() });
        let range = TextRange::empty(TextSize::from(0));

        let fb_symbol = Symbol::new(
            SymbolId::UNKNOWN,
            name,
            SymbolKind::FunctionBlock,
            type_id,
            range,
        );
        let fb_id = self.add_symbol(fb_symbol);

        for (param_name, type_id, direction) in params {
            self.add_parameter_symbol(fb_id, param_name, *type_id, *direction, range);
        }
    }

    fn add_parameter_symbol(
        &mut self,
        parent: SymbolId,
        name: &str,
        type_id: TypeId,
        direction: ParamDirection,
        range: TextRange,
    ) {
        let mut symbol = Symbol::new(
            SymbolId::UNKNOWN,
            name,
            SymbolKind::Parameter { direction },
            type_id,
            range,
        );
        symbol.parent = Some(parent);
        self.add_symbol_raw(symbol);
    }
}
