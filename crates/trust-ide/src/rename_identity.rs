//! What a rename renames, in the terms of paths outside the program (specification 39, §7.8): the
//! HMI bindings name variables by instance paths such as `Main.P101.Running`, so a rename reaches
//! them through the declaring type and the name.

use smol_str::SmolStr;
use text_size::TextSize;
use trust_hir::db::FileId;
use trust_hir::symbols::{Symbol, SymbolKind, SymbolTable};
use trust_hir::Database;

use crate::util::{resolve_target_at_position, ResolvedTarget};

/// What a rename at a position renames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameIdentity {
    /// A variable a program, function block or class declares, with that type's qualified name.
    Member {
        /// The declaring type, for example `Pump` or `Lib.Pump`.
        owner: SmolStr,
        /// The variable's name.
        name: SmolStr,
    },
    /// A structure field, with the structure type's qualified name.
    Field {
        /// The structure type.
        owner: SmolStr,
        /// The field's name.
        name: SmolStr,
    },
    /// A global variable or a configured program instance: the first part of a path.
    Root {
        /// Its name.
        name: SmolStr,
    },
    /// A program, whose instance carries its name when no configuration names one.
    Program {
        /// The program's name.
        name: SmolStr,
    },
}

/// The identity of what a rename at `position` renames; `None` where no path can reach it, as
/// for a function's or a method's own variables.
pub fn rename_identity(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameIdentity> {
    match resolve_target_at_position(db, file_id, position)? {
        ResolvedTarget::Field(field) => Some(RenameIdentity::Field {
            owner: field.type_name?,
            name: field.name,
        }),
        ResolvedTarget::Symbol(symbol_id) => {
            // The declaration itself, in the file that declares it.
            let local = db.file_symbols_with_project(file_id);
            let found = local.get(symbol_id)?;
            let (symbols, symbol_id) = match found.origin {
                Some(origin) => (
                    db.file_symbols_with_project(origin.file_id),
                    origin.symbol_id,
                ),
                None => (local.clone(), symbol_id),
            };
            let symbol = symbols.get(symbol_id)?;
            identity_of(&symbols, symbol)
        }
    }
}

fn identity_of(symbols: &SymbolTable, symbol: &Symbol) -> Option<RenameIdentity> {
    match symbol.kind {
        SymbolKind::Variable { .. } | SymbolKind::Parameter { .. } | SymbolKind::Constant => {
            let parent = symbol.parent.and_then(|parent| symbols.get(parent));
            match parent.map(|parent| &parent.kind) {
                Some(SymbolKind::Program | SymbolKind::FunctionBlock | SymbolKind::Class) => {
                    Some(RenameIdentity::Member {
                        owner: qualified_name(symbols, parent?),
                        name: symbol.name.clone(),
                    })
                }
                None
                | Some(SymbolKind::Configuration | SymbolKind::Resource | SymbolKind::Namespace) => {
                    Some(RenameIdentity::Root {
                        name: symbol.name.clone(),
                    })
                }
                _ => None,
            }
        }
        SymbolKind::ProgramInstance => Some(RenameIdentity::Root {
            name: symbol.name.clone(),
        }),
        SymbolKind::Program => Some(RenameIdentity::Program {
            name: symbol.name.clone(),
        }),
        _ => None,
    }
}

/// A type's name with the namespaces it is declared in, for example `Lib.Pump`.
fn qualified_name(symbols: &SymbolTable, symbol: &Symbol) -> SmolStr {
    let mut parts = vec![symbol.name.clone()];
    let mut parent = symbol.parent;
    // Bounded, so a malformed table cannot loop.
    for _ in 0..64 {
        let Some(namespace) = parent
            .and_then(|id| symbols.get(id))
            .filter(|candidate| candidate.kind == SymbolKind::Namespace)
        else {
            break;
        };
        parts.push(namespace.name.clone());
        parent = namespace.parent;
    }
    parts.reverse();
    SmolStr::new(parts.join("."))
}
