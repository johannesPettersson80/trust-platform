//! A compiled program's variables as a computer's protocols bind to them by name (specification 44
//! §13.3): a client's point binds a global, an MQTT tag a program instance's variable. Each is found
//! through the symbol table, with the shape of the type the binding is checked against, and
//! whether it is retained.

use smol_str::SmolStr;
use trust_runtime_core::native::image::{
    Image, SymbolRecord, NONE, SYMBOL_PROGRAM, SYMBOL_RETAINED,
};
use trust_runtime_core::native::memory::{type_name, type_shape, TypeShape};

/// A variable of a compiled program, as a protocol binds to it by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedVariable {
    /// The index of the variable's symbol.
    pub symbol: u32,
    /// The declared full name: a global's name, `Main.Speed` for a program instance's variable.
    pub name: SmolStr,
    /// The type's IEC name.
    pub type_name: SmolStr,
    /// The type's shape.
    pub shape: TypeShape,
    /// Whether the variable is retained.
    pub retained: bool,
}

/// The global of `image` that `name` names, compared without regard to case; none for a name that
/// names no global, a program instance's variable among them.
pub(super) fn global(image: &Image<'_>, name: &str) -> Option<NamedVariable> {
    named(image, name, |parent| parent.is_none())
}

/// The variable of a program instance of `image` that `name`, `Instance.Variable`, names, compared
/// without regard to case; none for a name that names no program instance's own variable.
pub(super) fn program_variable(image: &Image<'_>, name: &str) -> Option<NamedVariable> {
    named(image, name, |parent| {
        parent.is_some_and(|parent| parent.flags & SYMBOL_PROGRAM != 0)
    })
}

/// The variable `name` names whose parent `belongs` takes, none for a global.
fn named(
    image: &Image<'_>,
    name: &str,
    belongs: impl Fn(Option<&SymbolRecord>) -> bool,
) -> Option<NamedVariable> {
    let index = image.find_variable(name).ok()?;
    let symbols = image.table::<SymbolRecord>().ok()?;
    let symbol = symbols.get(index)?;
    let parent = if symbol.parent == NONE {
        None
    } else {
        Some(symbols.get(symbol.parent)?)
    };
    if symbol.flags & SYMBOL_PROGRAM != 0 || !belongs(parent.as_ref()) {
        return None;
    }
    Some(NamedVariable {
        symbol: index,
        name: SmolStr::new(image.string(symbol.display)?),
        type_name: type_name(image, symbol.type_index),
        shape: type_shape(image, symbol.type_index).ok()?,
        retained: symbol.flags & SYMBOL_RETAINED != 0,
    })
}
