use super::*;

mod assertions;
mod bit;
mod comparison;
mod conversions;
mod exprs;
mod helpers;
mod numeric;
mod selection;
mod string;
mod time;
mod validate;

pub(in crate::type_check) use helpers::is_execution_param;

pub(crate) fn is_standard_function_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if is_standard_conversion_function_name(&upper) {
        return true;
    }
    matches!(
        upper.as_str(),
        "ABS"
            | "SQRT"
            | "LN"
            | "LOG"
            | "EXP"
            | "SIN"
            | "COS"
            | "TAN"
            | "ASIN"
            | "ACOS"
            | "ATAN"
            | "ATAN2"
            | "ADD"
            | "SUB"
            | "MUL"
            | "DIV"
            | "MOD"
            | "EXPT"
            | "MOVE"
            | "SHL"
            | "SHR"
            | "ROL"
            | "ROR"
            | "AND"
            | "OR"
            | "XOR"
            | "NOT"
            | "SEL"
            | "MAX"
            | "MIN"
            | "LIMIT"
            | "MUX"
            | "GT"
            | "GE"
            | "EQ"
            | "LE"
            | "LT"
            | "NE"
            | "IS_VALID"
            | "IS_VALID_BCD"
            | "ASSERT_TRUE"
            | "ASSERT_FALSE"
            | "ASSERT_EQUAL"
            | "ASSERT_NOT_EQUAL"
            | "ASSERT_GREATER"
            | "ASSERT_LESS"
            | "ASSERT_GREATER_OR_EQUAL"
            | "ASSERT_LESS_OR_EQUAL"
            | "ASSERT_NEAR"
            | "LEN"
            | "LEFT"
            | "RIGHT"
            | "MID"
            | "CONCAT"
            | "INSERT"
            | "DELETE"
            | "REPLACE"
            | "FIND"
            | "TIME"
            | "CURRENT_DT"
            | "ADD_TIME"
            | "ADD_LTIME"
            | "ADD_TOD_TIME"
            | "ADD_LTOD_LTIME"
            | "ADD_DT_TIME"
            | "ADD_LDT_LTIME"
            | "SUB_TIME"
            | "SUB_LTIME"
            | "SUB_DATE_DATE"
            | "SUB_LDATE_LDATE"
            | "SUB_TOD_TIME"
            | "SUB_LTOD_LTIME"
            | "SUB_TOD_TOD"
            | "SUB_LTOD_LTOD"
            | "SUB_DT_TIME"
            | "SUB_LDT_LTIME"
            | "SUB_DT_DT"
            | "SUB_LDT_LDT"
            | "MUL_TIME"
            | "MUL_LTIME"
            | "DIV_TIME"
            | "DIV_LTIME"
            | "CONCAT_DATE_TOD"
            | "CONCAT_DATE_LTOD"
            | "CONCAT_DATE"
            | "CONCAT_TOD"
            | "CONCAT_LTOD"
            | "CONCAT_DT"
            | "CONCAT_LDT"
            | "SPLIT_DATE"
            | "SPLIT_TOD"
            | "SPLIT_LTOD"
            | "SPLIT_DT"
            | "SPLIT_LDT"
            | "DAY_OF_WEEK"
    )
}

fn is_standard_conversion_function_name(name: &str) -> bool {
    if name == "TRUNC" {
        return true;
    }
    if let Some(target) = name.strip_prefix("TRUNC_") {
        return TypeId::from_builtin_name(target).is_some();
    }
    if let Some((source, target)) = name.split_once("_TRUNC_") {
        return TypeId::from_builtin_name(source).is_some()
            && TypeId::from_builtin_name(target).is_some();
    }
    if let Some(target) = name.strip_prefix("TO_BCD_") {
        return TypeId::from_builtin_name(target).is_some();
    }
    if let Some((target, source)) = name.split_once("_TO_BCD_") {
        return TypeId::from_builtin_name(target).is_some()
            && TypeId::from_builtin_name(source).is_some();
    }
    if let Some(target) = name.strip_prefix("BCD_TO_") {
        return TypeId::from_builtin_name(target).is_some();
    }
    if let Some((source, target)) = name.split_once("_BCD_TO_") {
        return TypeId::from_builtin_name(source).is_some()
            && TypeId::from_builtin_name(target).is_some();
    }
    if let Some(target) = name.strip_prefix("TO_") {
        return TypeId::from_builtin_name(target).is_some();
    }
    name.split_once("_TO_").is_some_and(|(source, target)| {
        TypeId::from_builtin_name(source).is_some() && TypeId::from_builtin_name(target).is_some()
    })
}

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(super) fn infer_standard_function_call(
        &mut self,
        name: &str,
        node: &SyntaxNode,
    ) -> Option<TypeId> {
        let upper = name.to_ascii_uppercase();
        if !is_standard_function_name(&upper) {
            return None;
        }
        if let Some(result) = self.infer_conversion_function_call(&upper, node) {
            return Some(result);
        }

        let result =
            match upper.as_str() {
                "ABS" => self.infer_unary_numeric_call(node),
                "SQRT" | "LN" | "LOG" | "EXP" | "SIN" | "COS" | "TAN" | "ASIN" | "ACOS"
                | "ATAN" => self.infer_unary_real_call(node),
                "ATAN2" => self.infer_atan2_call(node),
                "ADD" => self.infer_add_call(node),
                "SUB" => self.infer_sub_call(node),
                "MUL" => self.infer_mul_call(node),
                "DIV" => self.infer_div_call(node),
                "MOD" => self.infer_mod_call(node),
                "EXPT" => self.infer_expt_call(node),
                "MOVE" => self.infer_move_call(node),
                "SHL" | "SHR" | "ROL" | "ROR" => self.infer_bit_shift_call(node, &upper),
                "AND" | "OR" | "XOR" => self.infer_variadic_bitwise_call(node),
                "NOT" => self.infer_not_call(node),
                "SEL" => self.infer_sel_call(node),
                "MAX" | "MIN" => self.infer_min_max_call(node),
                "LIMIT" => self.infer_limit_call(node),
                "MUX" => self.infer_mux_call(node),
                "GT" | "GE" | "EQ" | "LE" | "LT" | "NE" => self.infer_comparison_call(node, &upper),
                "IS_VALID" => self.infer_is_valid_call(node),
                "IS_VALID_BCD" => self.infer_is_valid_bcd_call(node),
                "ASSERT_TRUE" => self.infer_assert_true_call(node),
                "ASSERT_FALSE" => self.infer_assert_false_call(node),
                "ASSERT_EQUAL" => self.infer_assert_equal_call(node),
                "ASSERT_NOT_EQUAL" => self.infer_assert_not_equal_call(node),
                "ASSERT_GREATER" => self.infer_assert_greater_call(node),
                "ASSERT_LESS" => self.infer_assert_less_call(node),
                "ASSERT_GREATER_OR_EQUAL" => self.infer_assert_greater_or_equal_call(node),
                "ASSERT_LESS_OR_EQUAL" => self.infer_assert_less_or_equal_call(node),
                "ASSERT_NEAR" => self.infer_assert_near_call(node),
                "LEN" => self.infer_len_call(node),
                "LEFT" | "RIGHT" => self.infer_left_right_call(node, &upper),
                "MID" => self.infer_mid_call(node),
                "CONCAT" => self.infer_concat_call(node),
                "INSERT" => self.infer_insert_call(node),
                "DELETE" => self.infer_delete_call(node),
                "REPLACE" => self.infer_replace_call(node),
                "FIND" => self.infer_find_call(node),
                "TIME" => self.infer_time_call(node),
                "CURRENT_DT" => self.infer_current_dt_call(node),
                "ADD_TIME" | "ADD_LTIME" | "ADD_TOD_TIME" | "ADD_LTOD_LTIME" | "ADD_DT_TIME"
                | "ADD_LDT_LTIME" | "SUB_TIME" | "SUB_LTIME" | "SUB_DATE_DATE"
                | "SUB_LDATE_LDATE" | "SUB_TOD_TIME" | "SUB_LTOD_LTIME" | "SUB_TOD_TOD"
                | "SUB_LTOD_LTOD" | "SUB_DT_TIME" | "SUB_LDT_LTIME" | "SUB_DT_DT"
                | "SUB_LDT_LDT" => self.infer_time_named_arith_call(node, &upper),
                "MUL_TIME" | "MUL_LTIME" | "DIV_TIME" | "DIV_LTIME" => {
                    self.infer_time_named_mul_div_call(node, &upper)
                }
                "CONCAT_DATE_TOD" | "CONCAT_DATE_LTOD" | "CONCAT_DATE" | "CONCAT_TOD"
                | "CONCAT_LTOD" | "CONCAT_DT" | "CONCAT_LDT" => {
                    self.infer_concat_date_time_call(node, &upper)
                }
                "SPLIT_DATE" | "SPLIT_TOD" | "SPLIT_LTOD" | "SPLIT_DT" | "SPLIT_LDT" => {
                    self.infer_split_date_time_call(node, &upper)
                }
                "DAY_OF_WEEK" => self.infer_day_of_week_call(node),
                _ => unreachable!("recognized conversion returned no inference result"),
            };

        Some(result)
    }
}
