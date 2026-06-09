fn standard_signature(name: &str, arg_count: usize) -> Option<SignatureInfo> {
    let upper = name.to_ascii_uppercase();

    if let Some(signature) = conversion_signature(&upper) {
        return Some(signature);
    }

    let (params, return_type) = match upper.as_str() {
        // Numeric
        "ABS" => (vec![param("IN", TypeId::ANY_NUM)], None),
        "SQRT" | "LN" | "LOG" | "EXP" | "SIN" | "COS" | "TAN" | "ASIN" | "ACOS" | "ATAN" => {
            (vec![param("IN", TypeId::ANY_REAL)], None)
        }
        "ATAN2" => (
            vec![param("Y", TypeId::ANY_REAL), param("X", TypeId::ANY_REAL)],
            None,
        ),
        "ADD" => (variadic_in("IN", arg_count, 2, TypeId::ANY), None),
        "SUB" => (fixed_in("IN", 2, TypeId::ANY), None),
        "MUL" => (variadic_in("IN", arg_count, 2, TypeId::ANY), None),
        "DIV" => (fixed_in("IN", 2, TypeId::ANY), None),
        "MOD" => (fixed_in("IN", 2, TypeId::ANY_NUM), None),
        "EXPT" => (
            vec![
                param("IN1", TypeId::ANY_REAL),
                param("IN2", TypeId::ANY_NUM),
            ],
            None,
        ),
        "MOVE" => (vec![param("IN", TypeId::ANY)], None),

        // Bit
        "SHL" | "SHR" | "ROL" | "ROR" => (
            vec![param("IN", TypeId::ANY_BIT), param("N", TypeId::ANY_INT)],
            None,
        ),
        "AND" | "OR" | "XOR" => (variadic_in("IN", arg_count, 2, TypeId::ANY_BIT), None),
        "NOT" => (vec![param("IN", TypeId::ANY_BIT)], None),

        // Selection
        "SEL" => (
            vec![
                param("G", TypeId::BOOL),
                param("IN0", TypeId::ANY),
                param("IN1", TypeId::ANY),
            ],
            None,
        ),
        "MAX" | "MIN" => (
            variadic_in("IN", arg_count, 2, TypeId::ANY_ELEMENTARY),
            None,
        ),
        "LIMIT" => (
            vec![
                param("MN", TypeId::ANY_ELEMENTARY),
                param("IN", TypeId::ANY_ELEMENTARY),
                param("MX", TypeId::ANY_ELEMENTARY),
            ],
            None,
        ),
        "MUX" => (mux_params(arg_count), None),

        // Comparison
        "GT" | "GE" | "EQ" | "LE" | "LT" => (
            variadic_in("IN", arg_count, 2, TypeId::ANY_ELEMENTARY),
            Some(TypeId::BOOL),
        ),
        "NE" => (
            fixed_in("IN", 2, TypeId::ANY_ELEMENTARY),
            Some(TypeId::BOOL),
        ),

        // Validate
        "IS_VALID" => (vec![param("IN", TypeId::ANY_REAL)], Some(TypeId::BOOL)),
        "IS_VALID_BCD" => (vec![param("IN", TypeId::ANY_BIT)], Some(TypeId::BOOL)),

        // String
        "LEN" => (vec![param("IN", TypeId::ANY_STRING)], Some(TypeId::INT)),
        "LEFT" | "RIGHT" => (
            vec![param("IN", TypeId::ANY_STRING), param("L", TypeId::ANY_INT)],
            None,
        ),
        "MID" => (
            vec![
                param("IN", TypeId::ANY_STRING),
                param("L", TypeId::ANY_INT),
                param("P", TypeId::ANY_INT),
            ],
            None,
        ),
        "CONCAT" => (variadic_in("IN", arg_count, 2, TypeId::ANY_STRING), None),
        "INSERT" => (
            vec![
                param("IN1", TypeId::ANY_STRING),
                param("IN2", TypeId::ANY_STRING),
                param("P", TypeId::ANY_INT),
            ],
            None,
        ),
        "DELETE" => (
            vec![
                param("IN", TypeId::ANY_STRING),
                param("L", TypeId::ANY_INT),
                param("P", TypeId::ANY_INT),
            ],
            None,
        ),
        "REPLACE" => (
            vec![
                param("IN1", TypeId::ANY_STRING),
                param("IN2", TypeId::ANY_STRING),
                param("L", TypeId::ANY_INT),
                param("P", TypeId::ANY_INT),
            ],
            None,
        ),
        "FIND" => (
            vec![
                param("IN1", TypeId::ANY_STRING),
                param("IN2", TypeId::ANY_STRING),
            ],
            Some(TypeId::INT),
        ),

        // Time math
        "ADD_TIME" => (time_binary(TypeId::TIME, TypeId::TIME), Some(TypeId::TIME)),
        "ADD_LTIME" => (
            time_binary(TypeId::LTIME, TypeId::LTIME),
            Some(TypeId::LTIME),
        ),
        "ADD_TOD_TIME" => (time_binary(TypeId::TOD, TypeId::TIME), Some(TypeId::TOD)),
        "ADD_LTOD_LTIME" => (time_binary(TypeId::LTOD, TypeId::LTIME), Some(TypeId::LTOD)),
        "ADD_DT_TIME" => (time_binary(TypeId::DT, TypeId::TIME), Some(TypeId::DT)),
        "ADD_LDT_LTIME" => (time_binary(TypeId::LDT, TypeId::LTIME), Some(TypeId::LDT)),
        "SUB_TIME" => (time_binary(TypeId::TIME, TypeId::TIME), Some(TypeId::TIME)),
        "SUB_LTIME" => (
            time_binary(TypeId::LTIME, TypeId::LTIME),
            Some(TypeId::LTIME),
        ),
        "SUB_DATE_DATE" => (time_binary(TypeId::DATE, TypeId::DATE), Some(TypeId::TIME)),
        "SUB_LDATE_LDATE" => (
            time_binary(TypeId::LDATE, TypeId::LDATE),
            Some(TypeId::LTIME),
        ),
        "SUB_TOD_TIME" => (time_binary(TypeId::TOD, TypeId::TIME), Some(TypeId::TOD)),
        "SUB_LTOD_LTIME" => (time_binary(TypeId::LTOD, TypeId::LTIME), Some(TypeId::LTOD)),
        "SUB_TOD_TOD" => (time_binary(TypeId::TOD, TypeId::TOD), Some(TypeId::TIME)),
        "SUB_LTOD_LTOD" => (time_binary(TypeId::LTOD, TypeId::LTOD), Some(TypeId::LTIME)),
        "SUB_DT_TIME" => (time_binary(TypeId::DT, TypeId::TIME), Some(TypeId::DT)),
        "SUB_LDT_LTIME" => (time_binary(TypeId::LDT, TypeId::LTIME), Some(TypeId::LDT)),
        "SUB_DT_DT" => (time_binary(TypeId::DT, TypeId::DT), Some(TypeId::TIME)),
        "SUB_LDT_LDT" => (time_binary(TypeId::LDT, TypeId::LDT), Some(TypeId::LTIME)),
        "MUL_TIME" => (
            vec![param("IN1", TypeId::TIME), param("IN2", TypeId::ANY_NUM)],
            Some(TypeId::TIME),
        ),
        "MUL_LTIME" => (
            vec![param("IN1", TypeId::LTIME), param("IN2", TypeId::ANY_NUM)],
            Some(TypeId::LTIME),
        ),
        "DIV_TIME" => (
            vec![param("IN1", TypeId::TIME), param("IN2", TypeId::ANY_NUM)],
            Some(TypeId::TIME),
        ),
        "DIV_LTIME" => (
            vec![param("IN1", TypeId::LTIME), param("IN2", TypeId::ANY_NUM)],
            Some(TypeId::LTIME),
        ),
        "CONCAT_DATE_TOD" => (
            vec![param("DATE", TypeId::DATE), param("TOD", TypeId::TOD)],
            Some(TypeId::DT),
        ),
        "CONCAT_DATE_LTOD" => (
            vec![param("DATE", TypeId::DATE), param("LTOD", TypeId::LTOD)],
            Some(TypeId::LDT),
        ),
        "CONCAT_DATE" => (
            vec![
                param("YEAR", TypeId::ANY_INT),
                param("MONTH", TypeId::ANY_INT),
                param("DAY", TypeId::ANY_INT),
            ],
            Some(TypeId::DATE),
        ),
        "CONCAT_TOD" => (
            vec![
                param("HOUR", TypeId::ANY_INT),
                param("MINUTE", TypeId::ANY_INT),
                param("SECOND", TypeId::ANY_INT),
                param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::TOD),
        ),
        "CONCAT_LTOD" => (
            vec![
                param("HOUR", TypeId::ANY_INT),
                param("MINUTE", TypeId::ANY_INT),
                param("SECOND", TypeId::ANY_INT),
                param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::LTOD),
        ),
        "CONCAT_DT" => (
            vec![
                param("YEAR", TypeId::ANY_INT),
                param("MONTH", TypeId::ANY_INT),
                param("DAY", TypeId::ANY_INT),
                param("HOUR", TypeId::ANY_INT),
                param("MINUTE", TypeId::ANY_INT),
                param("SECOND", TypeId::ANY_INT),
                param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::DT),
        ),
        "CONCAT_LDT" => (
            vec![
                param("YEAR", TypeId::ANY_INT),
                param("MONTH", TypeId::ANY_INT),
                param("DAY", TypeId::ANY_INT),
                param("HOUR", TypeId::ANY_INT),
                param("MINUTE", TypeId::ANY_INT),
                param("SECOND", TypeId::ANY_INT),
                param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::LDT),
        ),
        "SPLIT_DATE" => (
            vec![
                param("IN", TypeId::DATE),
                out_param("YEAR", TypeId::ANY_INT),
                out_param("MONTH", TypeId::ANY_INT),
                out_param("DAY", TypeId::ANY_INT),
            ],
            Some(TypeId::VOID),
        ),
        "SPLIT_TOD" => (
            vec![
                param("IN", TypeId::TOD),
                out_param("HOUR", TypeId::ANY_INT),
                out_param("MINUTE", TypeId::ANY_INT),
                out_param("SECOND", TypeId::ANY_INT),
                out_param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::VOID),
        ),
        "SPLIT_LTOD" => (
            vec![
                param("IN", TypeId::LTOD),
                out_param("HOUR", TypeId::ANY_INT),
                out_param("MINUTE", TypeId::ANY_INT),
                out_param("SECOND", TypeId::ANY_INT),
                out_param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::VOID),
        ),
        "SPLIT_DT" => (
            vec![
                param("IN", TypeId::DT),
                out_param("YEAR", TypeId::ANY_INT),
                out_param("MONTH", TypeId::ANY_INT),
                out_param("DAY", TypeId::ANY_INT),
                out_param("HOUR", TypeId::ANY_INT),
                out_param("MINUTE", TypeId::ANY_INT),
                out_param("SECOND", TypeId::ANY_INT),
                out_param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::VOID),
        ),
        "SPLIT_LDT" => (
            vec![
                param("IN", TypeId::LDT),
                out_param("YEAR", TypeId::ANY_INT),
                out_param("MONTH", TypeId::ANY_INT),
                out_param("DAY", TypeId::ANY_INT),
                out_param("HOUR", TypeId::ANY_INT),
                out_param("MINUTE", TypeId::ANY_INT),
                out_param("SECOND", TypeId::ANY_INT),
                out_param("MILLISECOND", TypeId::ANY_INT),
            ],
            Some(TypeId::VOID),
        ),
        "DAY_OF_WEEK" => (vec![param("IN", TypeId::DATE)], Some(TypeId::INT)),

        // Special calls
        "REF" => (vec![param("IN", TypeId::ANY)], None),
        "NEW" | "__NEW" => (vec![param("TYPE", TypeId::ANY)], None),
        "__DELETE" => (vec![param("IN", TypeId::ANY)], Some(TypeId::VOID)),
        _ => return None,
    };

    Some(SignatureInfo {
        name: SmolStr::new(name),
        params,
        return_type,
    })
}
