//! The type conversions (IEC §6.6.2.5.2 to §6.6.2.5.7, Tables 22 to 27; specification 07 §2;
//! specification 44 §6.8): between the numbers, the bit strings, the characters, the texts and the
//! date and time types, and to and from BCD, each with the runtime's rounding, spelling and faults
//! through the library.
//!
//! A program built with the CODESYS conversion rules, which its project's `vendor_profile` selects
//! (specification 07 §2.10), converts as the CODESYS Development System does: a real to an integer
//! or a bit string rounds half away from zero, a value outside the target keeps the low-order bits
//! of the target's width, two's complement, `REAL_TO_DWORD`, `LREAL_TO_LWORD`, `DWORD_TO_REAL` and
//! `LWORD_TO_LREAL` convert the number, and a bit string to `BOOL` tests for zero. Every other
//! conversion is the same under both rules.

use trust_syntax::syntax::SyntaxNode;

use super::call::Called;
use super::text::Text;
use super::Lowerer;
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;
use crate::stdlib::conversions::ConversionProfile;

/// The room of the text of a number: 20 bytes for an integer, `lib::REAL_TEXT` for a real.
const INTEGER_TEXT: u32 = 20;

impl Lowerer<'_, '_> {
    /// `SRC_TO_DST(value)` of the result type `ty`, `TRUNC(value)`, and the BCD conversions
    /// `SRC_BCD_TO_DST` and `SRC_TO_BCD_DST`.
    pub(super) fn conversion(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        value: &SyntaxNode,
        ty: Ty,
    ) -> Result<Called, NativeError> {
        let from = self.type_of(value)?;
        if let Kind::WStr(_) = self.info(from).kind {
            return self.read_wide(node, name, value, ty);
        }
        if let Kind::Str(_) = self.info(from).kind {
            if let Kind::WStr(_) = self.info(ty).kind {
                let (text, capacity) = self.text_ref(value)?;
                return Ok(self.widened(&text, capacity));
            }
            return self.read_text(node, name, value, ty);
        }
        let source = self.scalar(value)?;
        let Some(src) = self.info(from).scalar() else {
            return Err(self.unsupported(node, format!("{name} of a value that is no scalar")));
        };
        if let Kind::WStr(_) = self.info(ty).kind {
            // A `WCHAR` is the one code unit of its code; any other value is spelled as its
            // `STRING` is, then widened (specification 07 §2).
            if src == Elem::WChar {
                let temp = self.temp();
                self.line(&format!("let {temp} = WStr::<1>::new();"));
                self.line(&format!("lib::assign_wchar(cx, {temp}.view(), {source})?;"));
                return Ok(Called::Text(vec![Text::WStr {
                    reference: format!("&{temp}"),
                    capacity: 1,
                }]));
            }
            let Called::Text(parts) = self.spell(node, name, &source, src)? else {
                return Err(self.unsupported(node, format!("{name}: a text")));
            };
            let capacity = parts
                .iter()
                .map(Text::capacity)
                .fold(0u32, u32::saturating_add);
            let text = self.text_temp(&parts, capacity);
            return Ok(self.widened(&text, capacity));
        }
        if let Kind::Str(_) = self.info(ty).kind {
            return self.spell(node, name, &source, src);
        }
        let Some(dst) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, format!("{name} to a type that is no scalar")));
        };
        if name.contains("BCD") {
            return self.bcd(node, name, &source, src, dst);
        }
        let rust = dst.rust();
        let truncate = name == "TRUNC";
        let codesys = self.model.conversions == ConversionProfile::Codesys;
        let character = |elem: Elem| matches!(elem, Elem::Char | Elem::WChar);
        let converted = if src == dst || (src.is_duration() && dst.is_duration()) {
            source
        } else if let Some((call, faults)) = super::dates::date_conversion(src, dst, &source) {
            if faults {
                self.bind(&call)
            } else {
                call
            }
        } else if character(dst) {
            // A number or a bit string by its value, a negative one from its bits, so that it
            // is out of range; a `WCHAR` as a `CHAR` by its code (specification 07 §2).
            if src.is_real() || src == Elem::Bool {
                return Err(
                    self.unsupported(node, format!("{name}: {} as a character", src.name()))
                );
            }
            let width = if dst == Elem::Char { "char" } else { "wchar" };
            self.bind(&format!("lib::code_to_{width}(cx, {source} as u64)?"))
        } else if character(src) && dst.is_integer() {
            self.bind(&format!("lib::uint_to::<{rust}>(cx, u64::from({source}))?"))
        } else if (character(src) && dst.is_bits())
            || (src == Elem::Bool && (dst.is_integer() || dst.is_bits()))
        {
            // A character by its code, cut to the bit string; a `BOOL` as 0 or 1.
            format!("({source} as {rust})")
        } else if src.is_bits() && dst == Elem::Bool {
            // IEC Table 24 transfers the rightmost bit (specification 07 §2.2); the CODESYS rules
            // test the bit string for zero (§2.10).
            if codesys {
                format!("(({source}) != 0)")
            } else {
                format!("((({source}) & 1) != 0)")
            }
        } else if src.is_integer() && dst == Elem::Bool {
            // A vendor extension: `TRUE` when the number is not zero (specification 07 §2.9).
            format!("(({source}) != 0)")
        } else if src.is_real() && dst == Elem::Bool {
            // A vendor extension: `TRUE` when the number is not zero, a value that is not finite
            // an overflow (specification 07 §2.9).
            self.bind(&format!("lib::real_to_bool(cx, f64::from({source}))?"))
        } else if src == Elem::Bool && dst.is_real() {
            // A vendor extension: 1.0 for `TRUE`, 0.0 for `FALSE` (specification 07 §2.9).
            format!("(u8::from({source}) as {rust})")
        } else if src.is_integer() && dst.is_integer() {
            if codesys {
                // The low-order bits of the target's width, two's complement (§2.10).
                format!("({source} as {rust})")
            } else if src.is_signed() {
                self.bind(&format!("lib::sint_to::<{rust}>(cx, i64::from({source}))?"))
            } else {
                self.bind(&format!("lib::uint_to::<{rust}>(cx, u64::from({source}))?"))
            }
        } else if src.is_integer() && dst.is_real() {
            let (function, wide) = if src.is_signed() {
                ("sint", "i64")
            } else {
                ("uint", "u64")
            };
            let bits = if dst == Elem::Real { 32 } else { 64 };
            format!("lib::{function}_to_real{bits}({wide}::from({source}))")
        } else if src.is_real() && dst.is_integer() {
            if codesys {
                self.wrapping(&source, rust, truncate)
            } else {
                let rounding = if truncate { "Truncate" } else { "Nearest" };
                self.bind(&format!(
                    "lib::real_to_int::<{rust}>(cx, f64::from({source}), lib::Rounding::{rounding})?"
                ))
            }
        } else if src == Elem::Real && dst == Elem::LReal {
            self.bind(&format!("lib::real32_to_real::<f64>(cx, {source})?"))
        } else if src == Elem::LReal && dst == Elem::Real {
            self.bind(&format!("lib::real64_to_real32::<f32>(cx, {source})?"))
        } else if (src.is_bits() || src.is_integer()) && (dst.is_bits() || dst.is_integer()) {
            // A bit string converts by its bits: cut to the destination's width or extended with
            // zeros, read with the destination's sign (specification 07 §2).
            format!("({source} as {rust})")
        } else if src == Elem::DWord && dst == Elem::Real {
            if codesys {
                // The number of the bit string (§2.10).
                format!("lib::uint_to_real32(u64::from({source}))")
            } else {
                self.bind(&format!("lib::bits_to_real32(cx, {source})?"))
            }
        } else if src == Elem::LWord && dst == Elem::LReal {
            if codesys {
                format!("lib::uint_to_real64({source})")
            } else {
                self.bind(&format!("lib::bits_to_real64(cx, {source})?"))
            }
        } else if src == Elem::Real && dst == Elem::DWord
            || src == Elem::LReal && dst == Elem::LWord
        {
            if codesys {
                // The number, rounded and kept to the bit string's width (§2.10).
                self.wrapping(&source, rust, truncate)
            } else {
                format!("{source}.to_bits()")
            }
        } else if src.is_real() && dst.is_bits() {
            // A vendor extension: the number as the unsigned integer of the bit string's width,
            // rounded to the nearest and an overflow outside it; `REAL_TO_DWORD` and
            // `LREAL_TO_LWORD` above are the Table 25 transfers (specification 07 §2.9). The
            // CODESYS rules keep the low-order bits (§2.10).
            if codesys {
                self.wrapping(&source, rust, truncate)
            } else {
                self.bind(&format!(
                    "lib::real_to_int::<{rust}>(cx, f64::from({source}), lib::Rounding::Nearest)?"
                ))
            }
        } else if src.is_duration() && dst == Elem::DWord {
            self.bind(&format!("lib::time_to_dword(cx, {source})?"))
        } else if src == Elem::DWord && dst.is_duration() {
            format!("lib::dword_to_time({source})")
        } else {
            return Err(self.unsupported(node, format!("{name}: {} to {}", src.name(), dst.name())));
        };
        Ok(Called::Scalar(converted))
    }

    /// The real `source` as the integer or bit string of Rust type `rust` by the CODESYS rules
    /// (specification 07 §2.10): rounded half away from zero, or toward zero for `TRUNC`, with the
    /// low-order bits of the target's width; a value that is not finite is an overflow.
    fn wrapping(&mut self, source: &str, rust: &str, truncate: bool) -> String {
        let rounding = if truncate { "Truncate" } else { "HalfAway" };
        self.bind(&format!(
            "lib::real_to_int_wrapping::<{rust}>(cx, f64::from({source}), lib::Rounding::{rounding})?"
        ))
    }

    /// `SRC_TO_STRING(source)`: the text of a number, a bit string or a character, in a
    /// temporary that holds every text of its type (specification 07 §2.8).
    fn spell(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        source: &str,
        src: Elem,
    ) -> Result<Called, NativeError> {
        let (call, capacity) = if src.is_signed() {
            (
                format!("lib::write_int(TEXT.view(), i64::from({source}))"),
                INTEGER_TEXT,
            )
        } else if src.is_unsigned()
            || matches!(src, Elem::Byte | Elem::Word | Elem::DWord | Elem::LWord)
        {
            (
                format!("lib::write_uint(TEXT.view(), u64::from({source}))"),
                INTEGER_TEXT,
            )
        } else if src.is_real() {
            (
                format!("lib::write_real(TEXT.view(), f64::from({source}))"),
                u32::try_from(trust_plc_lib::REAL_TEXT).unwrap_or(u32::MAX),
            )
        } else if src == Elem::Char {
            // A `CHAR` is the character of its code, U+0000 to U+00FF, in UTF-8.
            (format!("lib::write_char(TEXT.view(), {source})"), 2)
        } else if src == Elem::WChar {
            (format!("lib::write_wchar(cx, TEXT.view(), {source})?"), 3)
        } else {
            return Err(self.unsupported(node, format!("{name}: {} as text", src.name())));
        };
        let temp = self.temp();
        self.line(&format!("let {temp} = Str::<{capacity}>::new();"));
        self.line(&format!("{};", call.replace("TEXT", &temp)));
        Ok(Called::Text(vec![Text::Str {
            reference: format!("&{temp}"),
            capacity,
        }]))
    }

    /// The `STRING` `text` of `capacity` bytes as a `WSTRING`: at most as many code units as it
    /// has bytes.
    fn widened(&mut self, text: &str, capacity: u32) -> Called {
        let temp = self.temp();
        self.line(&format!("let {temp} = WStr::<{capacity}>::new();"));
        self.line(&format!(
            "lib::string_to_wstring({temp}.view(), ({text}).view());"
        ));
        Called::Text(vec![Text::WStr {
            reference: format!("&{temp}"),
            capacity,
        }])
    }

    /// `WSTRING_TO_DST(value)`: the `WSTRING` itself, its text as a `STRING`, at most three bytes
    /// of UTF-8 for each code unit, or what that `STRING` spells (specification 07 §2).
    fn read_wide(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        value: &SyntaxNode,
        ty: Ty,
    ) -> Result<Called, NativeError> {
        let (text, capacity) = self.text_ref(value)?;
        if let Kind::WStr(_) = self.info(ty).kind {
            return Ok(Called::Text(vec![Text::WStr {
                reference: text,
                capacity,
            }]));
        }
        let narrow = capacity.saturating_mul(3).min(65_535);
        let temp = self.temp();
        self.line(&format!("let {temp} = Str::<{narrow}>::new();"));
        self.line(&format!(
            "lib::wstring_to_string(cx, {temp}.view(), ({text}).view())?;"
        ));
        self.read_text_ref(node, name, format!("&{temp}"), narrow, ty)
    }

    /// `STRING_TO_DST(value)`: the number or the character a text spells, read as the runtime
    /// reads it in a copy of the text the size of its capacity (specification 07 §2.8).
    fn read_text(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        value: &SyntaxNode,
        ty: Ty,
    ) -> Result<Called, NativeError> {
        let (text, capacity) = self.text_ref(value)?;
        self.read_text_ref(node, name, text, capacity, ty)
    }

    /// [`Self::read_text`] of the `STRING` `text` of `capacity` bytes.
    fn read_text_ref(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        text: String,
        capacity: u32,
        ty: Ty,
    ) -> Result<Called, NativeError> {
        if let Kind::Str(_) = self.info(ty).kind {
            return Ok(Called::Text(vec![Text::Str {
                reference: text,
                capacity,
            }]));
        }
        let Some(dst) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, format!("{name} to a type that is no scalar")));
        };
        let rust = dst.rust();
        let converted = if dst.is_integer() {
            let copy = self.scratch(capacity);
            let parsed = self.bind(&format!(
                "lib::parse_int(cx, ({text}).view(), &mut {copy})?"
            ));
            self.bind(&format!("lib::wide_to::<{rust}>(cx, {parsed})?"))
        } else if dst.is_real() {
            let copy = self.scratch(capacity);
            let parsed = self.bind(&format!(
                "lib::parse_real(cx, ({text}).view(), &mut {copy})?"
            ));
            if dst == Elem::Real {
                self.bind(&format!("lib::real64_to_real32::<f32>(cx, {parsed})?"))
            } else {
                parsed
            }
        } else if dst == Elem::Char {
            self.bind(&format!("lib::text_to_char(cx, ({text}).view())?"))
        } else {
            return Err(self.unsupported(node, format!("{name}: a text to {}", dst.name())));
        };
        Ok(Called::Scalar(converted))
    }

    /// A fresh `[u8; capacity]` the library copies a text into to read it.
    fn scratch(&mut self, capacity: u32) -> String {
        let copy = self.temp();
        self.line(&format!("let mut {copy} = [0u8; {capacity}];"));
        copy
    }

    /// `USINT_TO_BCD_BYTE(value)` and `BYTE_BCD_TO_USINT(value)` (IEC Table 25): an unsigned
    /// integer as the decimal digits of a bit string, four bits each, and back.
    fn bcd(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        source: &str,
        src: Elem,
        dst: Elem,
    ) -> Result<Called, NativeError> {
        let digits = |elem: Elem| match elem {
            Elem::Byte => Some(2),
            Elem::Word => Some(4),
            Elem::DWord => Some(8),
            Elem::LWord => Some(16),
            _ => None,
        };
        let rust = dst.rust();
        let converted = match (digits(src), digits(dst)) {
            (None, Some(count)) if src.is_unsigned() => self.bind(&format!(
                "lib::to_bcd(cx, u64::from({source}), {count})? as {rust}"
            )),
            (Some(count), None) if dst.is_unsigned() => {
                let decoded =
                    self.bind(&format!("lib::from_bcd(cx, u64::from({source}), {count})?"));
                self.bind(&format!("lib::uint_to::<{rust}>(cx, {decoded})?"))
            }
            _ => {
                return Err(
                    self.unsupported(node, format!("{name}: {} to {}", src.name(), dst.name()))
                );
            }
        };
        Ok(Called::Scalar(converted))
    }
}
