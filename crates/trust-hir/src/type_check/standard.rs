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

pub(in crate::type_check) use helpers::is_execution_param;

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(super) fn infer_standard_function_call(
        &mut self,
        name: &str,
        node: &SyntaxNode,
    ) -> Option<TypeId> {
        let upper = name.to_ascii_uppercase();
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
                "ASSERT_TRUE" => self.infer_assert_true_call(node),
                "ASSERT_FALSE" => self.infer_assert_false_call(node),
                "ASSERT_EQUAL" => self.infer_assert_equal_call(node),
                "ASSERT_NEAR" => self.infer_assert_near_call(node),
                "LEN" => self.infer_len_call(node),
                "LEFT" | "RIGHT" => self.infer_left_right_call(node, &upper),
                "MID" => self.infer_mid_call(node),
                "CONCAT" => self.infer_concat_call(node),
                "INSERT" => self.infer_insert_call(node),
                "DELETE" => self.infer_delete_call(node),
                "REPLACE" => self.infer_replace_call(node),
                "FIND" => self.infer_find_call(node),
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
                _ => return None,
            };

        Some(result)
    }
}
