//! A `#[trust_sequence]` block as a whole (specification 48 RU-71, RU-72): the POU's sequence, the
//! one `async fn` that no other awaits, its children in the block's order, and the refusals that
//! need the block: a child awaited that the block does not hold, a sequence that awaits itself, two
//! sequences that no other awaits, a child's name no field of `_seq` can take, and a loop that a
//! child which can return without waiting would let run without a wait point.

use syn::ext::IdentExt as _;
use syn::spanned::Spanned;
use syn::{Error, ImplItem, ImplItemFn, ItemImpl, Type};

use super::{leaks, Awaited, Block, Sequence, DONE};

/// A sequence of the block as [`sequence`](super::sequence) reads it, with the child sequences its
/// wait points await.
type Read = (Sequence, Vec<Awaited>);

/// Reads the `#[trust_sequence]` block `item`: the POU's sequence, the one `async fn` that no other
/// awaits, and the child sequences in the block's order (RU-71, RU-72).
///
/// # Errors
///
/// A refusal at its place: a block of a trait or of no struct, one without an `async fn`, a sequence
/// that [`sequence`](super::sequence) refuses, a child awaited that is no `async fn` of the block, a
/// sequence that awaits itself, directly or through other children, two `async fn`s that no other
/// awaits, a loop that a child which can return without waiting would let spin, and a child whose
/// name no field of `_seq` can take.
pub fn block(item: &ItemImpl) -> Result<Block, Error> {
    let owner = owner(item)?;
    let functions: Vec<&ImplItemFn> = item
        .items
        .iter()
        .filter_map(|member| match member {
            ImplItem::Fn(function) if function.sig.asyncness.is_some() => Some(function),
            _ => None,
        })
        .collect();
    if functions.is_empty() {
        return Err(Error::new(
            item.impl_token.span(),
            "a `#[trust_sequence]` block holds the POU's sequence, an `async fn name(&mut self, s: \
             &mut Seq) -> Result<(), Fault>` (specification 48 RU-70)",
        ));
    }
    let read: Vec<Read> = functions
        .iter()
        .copied()
        .map(super::read)
        .collect::<Result<_, _>>()?;
    awaits_its_children(&read)?;
    let sequence = pou_sequence(item, &functions, &read)?;
    loops_wait(&functions, &read)?;
    let mut children: Vec<Sequence> = Vec::new();
    for (function, (child, _)) in functions.iter().zip(&read) {
        if child.name == sequence.name {
            continue;
        }
        child_name(function, &sequence, &children)?;
        children.push(child.clone());
    }
    Ok(Block {
        owner,
        sequence,
        children,
    })
}

/// The struct the block is the `impl` block of, the POU's Rust item.
fn owner(item: &ItemImpl) -> Result<String, Error> {
    let owner = match (&item.trait_, &*item.self_ty) {
        (None, Type::Path(path)) if path.qself.is_none() => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.unraw().to_string()),
        _ => None,
    };
    owner.ok_or_else(|| {
        Error::new(
            item.impl_token.span(),
            "a `#[trust_sequence]` block is the `impl` block of a POU's struct, not a trait's \
             (specification 48 RU-71)",
        )
    })
}

/// Refuses a child awaited that is no sequence of the block, and a sequence that awaits itself,
/// directly or through other children: each sequence has one state.
fn awaits_its_children(read: &[Read]) -> Result<(), Error> {
    for (sequence, children) in read {
        for Awaited {
            child,
            place,
            arguments,
        } in children
        {
            let Some((awaited, _)) = read.iter().find(|(other, _)| other.name == *child) else {
                return Err(Error::new(
                    *place,
                    format!(
                        "`self.{child}(..)` awaits no sequence of this block: a child sequence is \
                         an `async fn` of the same `#[trust_sequence]` block (specification 48 \
                         RU-72)"
                    ),
                ));
            };
            if awaited.parameters.len() != *arguments {
                return Err(Error::new(
                    *place,
                    format!(
                        "`self.{child}(..)` passes {arguments} argument(s) after the context to the \
                         child sequence `{child}`, which takes {} parameter(s) (specification 48 \
                         RU-72)",
                        awaited.parameters.len()
                    ),
                ));
            }
            if *child == sequence.name || reaches(child, &sequence.name, read) {
                return Err(Error::new(
                    *place,
                    format!(
                        "`{}` awaits itself through `{child}`: each sequence has one state, so no \
                         sequence awaits itself, directly or through other children \
                         (specification 48 RU-72)",
                        sequence.name
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// The POU's sequence: the one sequence of the block that no other awaits.
fn pou_sequence(
    item: &ItemImpl,
    functions: &[&ImplItemFn],
    read: &[Read],
) -> Result<Sequence, Error> {
    let awaited = |name: &str| {
        read.iter()
            .any(|(other, _)| other.awaits.iter().any(|child| child == name))
    };
    let mut roots = functions
        .iter()
        .zip(read)
        .filter(|(_, (sequence, _))| !awaited(&sequence.name));
    let (Some((_, (first, _))), second) = (roots.next(), roots.next()) else {
        return Err(Error::new(
            item.impl_token.span(),
            "the block's sequences await each other: the POU's sequence is the one `async fn` that \
             no other awaits (specification 48 RU-72)",
        ));
    };
    if let Some((function, (other, _))) = second {
        return Err(Error::new(
            function.sig.ident.span(),
            format!(
                "`{}` and `{}` are both awaited by no sequence of the block: the POU's sequence is \
                 the one `async fn` that no other awaits, and every other a child that a sequence of \
                 the block awaits (specification 48 RU-72)",
                first.name, other.name
            ),
        ));
    }
    if !first.parameters.is_empty() {
        let place = functions
            .iter()
            .zip(read)
            .find(|(_, (sequence, _))| sequence.name == first.name)
            .and_then(|(function, _)| function.sig.inputs.iter().nth(2))
            .map_or(item.impl_token.span(), Spanned::span);
        return Err(Error::new(
            place,
            format!(
                "the POU's sequence `{}` takes no parameter but its context: the POU's activation \
                 runs it (specification 48 RU-72)",
                first.name
            ),
        ));
    }
    Ok(first.clone())
}

/// Refuses a loop of any sequence of the block that can go round without reaching a wait point,
/// counting as one only the children that wait before every way they return (RU-72): a child that
/// can return without waiting is no wait point, or a scan would never end.
fn loops_wait(functions: &[&ImplItemFn], read: &[Read]) -> Result<(), Error> {
    let mut waiting: Vec<String> = Vec::new();
    loop {
        let known = waiting.len();
        for (function, (sequence, _)) in functions.iter().zip(read) {
            if !waiting.contains(&sequence.name)
                && leaks::waits_before_returning(&function.block, &|child| {
                    waiting.iter().any(|name| name == child)
                })
            {
                waiting.push(sequence.name.clone());
            }
        }
        if waiting.len() == known {
            break;
        }
    }
    let waits = |child: &str| waiting.iter().any(|name| name == child);
    match functions
        .iter()
        .find_map(|function| leaks::spinning_loop(&function.block, &waits))
    {
        Some(place) => Err(Error::new(
            place,
            "this loop can run without reaching a wait point: a child sequence it awaits can \
             return without waiting, and every way through a loop awaits one, or a scan would \
             never end (specification 48 RU-72)",
        )),
        None => Ok(()),
    }
}

/// Whether the sequence named `from` awaits the one named `target`, directly or through other
/// children of `read`.
fn reaches(from: &str, target: &str, read: &[Read]) -> bool {
    let mut seen: Vec<&str> = vec![from];
    let mut work: Vec<&str> = vec![from];
    while let Some(name) = work.pop() {
        let Some((sequence, _)) = read.iter().find(|(sequence, _)| sequence.name == name) else {
            continue;
        };
        for child in &sequence.awaits {
            if child == target {
                return true;
            }
            if !seen.contains(&child.as_str()) {
                seen.push(child);
                work.push(child);
            }
        }
    }
    false
}

/// Refuses the child sequence `function` a name that no field of the state of the POU's
/// `sequence` can take: no IEC identifier, a reserved word, a leading underscore, which
/// `<POU>_SEQ_<CHILD>` would double, or a name of a field the state has already, `point`, `began`,
/// `done`, a local or an earlier child, in any case (RU-71).
fn child_name(
    function: &ImplItemFn,
    sequence: &Sequence,
    children: &[Sequence],
) -> Result<(), Error> {
    let ident = &function.sig.ident;
    let name = crate::variable_name(ident)?;
    if name.starts_with('_') {
        return Err(Error::new(
            ident.span(),
            format!(
                "a child sequence's name begins with a letter: its state is the field \
                 `_seq.{name}` of the structure `<POU>_SEQ_{}` (specification 48 RU-71)",
                name.to_ascii_uppercase()
            ),
        ));
    }
    let taken = ["point", "began", DONE]
        .into_iter()
        .chain(sequence.locals.iter().map(|local| local.name.as_str()))
        .chain(children.iter().map(|child| child.name.as_str()))
        .find(|field| field.eq_ignore_ascii_case(&name));
    if let Some(field) = taken {
        return Err(Error::new(
            ident.span(),
            format!(
                "the child sequence `{name}` keeps its state in the field `_seq.{name}`, which the \
                 state of `{}` has already as `{field}`: rename one (specification 48 RU-71)",
                sequence.name
            ),
        ));
    }
    Ok(())
}
