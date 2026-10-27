//! Texts (specification 02 §6; specification 07 §8; specification 44 §6.5, §7.6): a `STRING` or
//! `WSTRING` expression as parts, a literal's bytes or code units and the strings it reads, and
//! their store into a place, cut at its capacity.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::expr::Literal;
use super::{Lowerer, Place};
use crate::native::types::Kind;
use crate::native::NativeError;

/// A part of a text: a literal's bytes or a string's value, as `CONCAT` and an assignment take it.
pub(super) enum Text {
    Bytes(Vec<u8>),
    /// A reference to a `Str<N>` and its capacity `N`.
    Str {
        reference: String,
        capacity: u32,
    },
    /// The UTF-16 code units of a `WSTRING` literal.
    Units(Vec<u16>),
    /// A reference to a `WStr<N>` and its capacity `N`.
    WStr {
        reference: String,
        capacity: u32,
    },
}

impl Text {
    pub(super) fn piece(&self) -> String {
        match self {
            Self::Bytes(bytes) => format!("lib::Piece::Bytes({})", byte_string(bytes)),
            Self::Str { reference, .. } => format!("lib::Piece::Str(({reference}).view())"),
            Self::Units(units) => format!("lib::WPiece::Units({})", unit_string(units)),
            Self::WStr { reference, .. } => format!("lib::WPiece::WStr(({reference}).view())"),
        }
    }

    /// The most bytes, or code units of a `WSTRING`, the part can hold.
    pub(super) fn capacity(&self) -> u32 {
        match self {
            Self::Bytes(bytes) => u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            Self::Units(units) => u32::try_from(units.len()).unwrap_or(u32::MAX),
            Self::Str { capacity, .. } | Self::WStr { capacity, .. } => *capacity,
        }
    }

    /// Whether the part is of a `WSTRING`.
    pub(super) fn is_wide(&self) -> bool {
        matches!(self, Self::Units(_) | Self::WStr { .. })
    }

    /// Whether the part reads a string, which may be the one a store writes.
    fn is_reference(&self) -> bool {
        matches!(self, Self::Str { .. } | Self::WStr { .. })
    }
}

/// `parts` with adjacent literals joined into one: a text of literals only is then one byte
/// string, written by a call that takes it, and no array of pieces holds a pointer to a literal. A
/// constant array would be promoted to a static whose pointers a position-independent module could
/// reach only through relocations, which a module has none of (specification 44 §5.3).
fn joined(parts: &[Text]) -> Vec<Text> {
    let mut joined: Vec<Text> = Vec::with_capacity(parts.len());
    for part in parts {
        match (joined.last_mut(), part) {
            (Some(Text::Bytes(last)), Text::Bytes(bytes)) => {
                last.extend_from_slice(bytes);
                continue;
            }
            (Some(Text::Units(last)), Text::Units(units)) => {
                last.extend_from_slice(units);
                continue;
            }
            _ => {}
        }
        joined.push(match part {
            Text::Bytes(bytes) => Text::Bytes(bytes.clone()),
            Text::Units(units) => Text::Units(units.clone()),
            Text::Str {
                reference,
                capacity,
            } => Text::Str {
                reference: reference.clone(),
                capacity: *capacity,
            },
            Text::WStr {
                reference,
                capacity,
            } => Text::WStr {
                reference: reference.clone(),
                capacity: *capacity,
            },
        });
    }
    joined
}

/// `units`, UTF-16 code units, as a Rust slice of `u16`.
pub(in crate::native) fn unit_string(units: &[u16]) -> String {
    let units: Vec<String> = units.iter().map(|unit| format!("{unit:#06x}u16")).collect();
    format!("&[{}]", units.join(", "))
}

/// The longest text a string function or a concatenation may give, in bytes (specification 07
/// §8.1): a longer literal text is concatenated, which refuses it as the runtime does.
const MAX_TEXT_BYTES: usize = 65_535;

/// `bytes` as a Rust byte string literal.
pub(in crate::native) fn byte_string(bytes: &[u8]) -> String {
    let mut out = String::from("b\"");
    for byte in bytes {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(*byte)),
            other => out.push_str(&format!("\\x{other:02x}")),
        }
    }
    out.push('"');
    out
}

impl Lowerer<'_, '_> {
    /// The text of `node` as parts: a literal, a string, a string function's result.
    pub(super) fn text(&mut self, node: &SyntaxNode) -> Result<Vec<Text>, NativeError> {
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.text(&inner),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::Literal => {
                let ty = self.type_of(node)?;
                match self.literal_value(node, ty)? {
                    Literal::Text(bytes) => Ok(vec![Text::Bytes(bytes)]),
                    Literal::WText(units) => Ok(vec![Text::Units(units)]),
                    _ => Err(self.unsupported(node, "a literal that is no text")),
                }
            }
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                let place = self.place(node)?;
                self.text_of(node, &place)
            }
            SyntaxKind::CallExpr => self.call_text(node),
            other => Err(self.unsupported(node, format!("the text expression {other:?}"))),
        }
    }

    /// The text `place`, a `STRING` or a `WSTRING`, holds.
    pub(super) fn text_of(
        &self,
        node: &SyntaxNode,
        place: &Place,
    ) -> Result<Vec<Text>, NativeError> {
        match self.info(place.ty).kind {
            Kind::Str(capacity) => Ok(vec![Text::Str {
                reference: place.reference(),
                capacity,
            }]),
            Kind::WStr(capacity) => Ok(vec![Text::WStr {
                reference: place.reference(),
                capacity,
            }]),
            _ => Err(self.unsupported(node, "a value that is no text where a text is needed")),
        }
    }

    /// `place := value` for a `STRING` or a `WSTRING` place: the text cut at the place's capacity,
    /// in bytes or in code units, at the last whole character (specification 02 §6).
    pub(super) fn store_text(
        &mut self,
        place: &Place,
        value: &SyntaxNode,
    ) -> Result<(), NativeError> {
        let written = self.text(value)?;
        let parts = joined(&written);
        let target = place.reference();
        // A part after the first may read the target itself (`S := CONCAT('0', S)`), which the
        // concatenation has overwritten by then: the text is concatenated into a temporary of the
        // target's capacity, then assigned, so that the target takes the expression's value.
        if parts.iter().skip(1).any(Text::is_reference) {
            let capacity = match self.info(place.ty).kind {
                Kind::Str(capacity) | Kind::WStr(capacity) => capacity,
                _ => {
                    return Err(self.unsupported(value, "a text stored in a value of no text type"))
                }
            };
            let temp = self.text_temp(&written, capacity);
            let assign = if parts.iter().any(Text::is_wide) {
                "assign_wide"
            } else {
                "assign"
            };
            self.line(&format!(
                "lib::{assign}(({target}).view(), ({temp}).view());"
            ));
            return Ok(());
        }
        if parts.iter().any(Text::is_wide) {
            self.write_wide(&target, &parts, written.len());
            return Ok(());
        }
        match parts.as_slice() {
            // A literal is cut to the capacity; literals concatenated past the longest text are
            // refused as the concatenation refuses them.
            [Text::Bytes(bytes)] if written.len() == 1 || bytes.len() <= MAX_TEXT_BYTES => {
                self.line(&format!(
                    "lib::assign_bytes(({target}).view(), {});",
                    byte_string(bytes)
                ));
            }
            [Text::Str {
                reference: source, ..
            }] => {
                self.line(&format!(
                    "lib::assign(({target}).view(), ({source}).view());"
                ));
            }
            _ => {
                let pieces: Vec<String> = parts.iter().map(Text::piece).collect();
                self.line(&format!(
                    "lib::concat(cx, ({target}).view(), &[{}])?;",
                    pieces.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// The joined wide `parts`, of `written` parts before they were joined, written into the
    /// `WStr` `target`, cut at its capacity.
    pub(super) fn write_wide(&mut self, target: &str, parts: &[Text], written: usize) {
        match parts {
            [Text::Units(units)] if written == 1 || units.len() <= MAX_TEXT_BYTES => {
                self.line(&format!(
                    "lib::assign_units(({target}).view(), {});",
                    unit_string(units)
                ));
            }
            [Text::WStr {
                reference: source, ..
            }] => {
                self.line(&format!(
                    "lib::assign_wide(({target}).view(), ({source}).view());"
                ));
            }
            _ => {
                let pieces: Vec<String> = parts.iter().map(Text::piece).collect();
                self.line(&format!(
                    "lib::concat_wide(cx, ({target}).view(), &[{}])?;",
                    pieces.join(", ")
                ));
            }
        }
    }

    /// `parts` written into a fresh temporary `STRING[capacity]`, or `WSTRING[capacity]` for wide
    /// parts, whose reference is returned.
    pub(super) fn text_temp(&mut self, parts: &[Text], capacity: u32) -> String {
        if parts.iter().any(Text::is_wide) {
            let temp = self.temp();
            self.line(&format!("let {temp} = WStr::<{capacity}>::new();"));
            let written = parts.len();
            let parts = joined(parts);
            self.write_wide(&temp, &parts, written);
            return format!("&{temp}");
        }
        let temp = self.temp();
        self.line(&format!("let {temp} = Str::<{capacity}>::new();"));
        let written = parts.len();
        let parts = joined(parts);
        if let [Text::Bytes(bytes)] = parts.as_slice()
            && (written == 1 || bytes.len() <= MAX_TEXT_BYTES)
        {
            self.line(&format!(
                "lib::assign_bytes({temp}.view(), {});",
                byte_string(bytes)
            ));
            return format!("&{temp}");
        }
        let pieces: Vec<String> = parts.iter().map(Text::piece).collect();
        self.line(&format!(
            "lib::concat(cx, {temp}.view(), &[{}])?;",
            pieces.join(", ")
        ));
        format!("&{temp}")
    }

    pub(super) fn compare_texts(
        &mut self,
        node: &SyntaxNode,
        left: &SyntaxNode,
        right: &SyntaxNode,
        op: SyntaxKind,
    ) -> Result<String, NativeError> {
        let wide = self.is_wide_text(left);
        let (a, _) = self.text_ref(left)?;
        let (b, _) = self.text_ref(right)?;
        let compare = if wide { "compare_wide" } else { "compare" };
        let ordering = match op {
            SyntaxKind::Eq => "== core::cmp::Ordering::Equal",
            SyntaxKind::Neq => "!= core::cmp::Ordering::Equal",
            SyntaxKind::Lt => "== core::cmp::Ordering::Less",
            SyntaxKind::Gt => "== core::cmp::Ordering::Greater",
            SyntaxKind::LtEq => "!= core::cmp::Ordering::Greater",
            SyntaxKind::GtEq => "!= core::cmp::Ordering::Less",
            _ => return Err(self.unsupported(node, "this comparison of texts")),
        };
        Ok(format!(
            "(lib::{compare}(({a}).view(), ({b}).view()) {ordering})"
        ))
    }

    /// Whether `node` is a `WSTRING`.
    pub(super) fn is_wide_text(&mut self, node: &SyntaxNode) -> bool {
        self.type_of(node)
            .is_ok_and(|ty| matches!(self.info(ty).kind, Kind::WStr(_)))
    }
}
