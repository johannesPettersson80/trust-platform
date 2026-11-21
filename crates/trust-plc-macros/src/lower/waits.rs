//! A wait point of the lowered sequence (specification 48 RU-72): what an activation that resumes
//! it checks, `.within(t)` first, and the code of that check, which either returns to wait on or
//! goes on with the code after the wait point.

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Error, Expr, Ident};

use super::Lowering;

/// What an activation that resumes a wait point checks.
pub(super) enum Check {
    /// `s.next_cycle()`: over at once.
    NextCycle,
    /// `s.until(pred)`: over when `pred` holds for the instance.
    Until(Expr),
    /// `s.wait(t)`: over when `t` has passed.
    Wait(Expr),
    /// `s.all(..)`: over when each member has completed, at the same activation or before.
    All(Vec<Check>),
    /// `s.race(..)`: over when one member completes, the first in order winning.
    Race(Vec<Check>),
    /// `self.<child>(s)`: over when the child sequence has returned, which it runs.
    Child(Ident),
}

/// A wait point: what it checks, and the limit of its `.within(t)`.
pub(super) struct WaitPoint {
    pub(super) check: Check,
    pub(super) within: Option<Expr>,
}

impl Lowering {
    /// What the wait point `base`, the expression before `.await`, checks.
    pub(super) fn wait_of(&self, base: &Expr) -> Result<WaitPoint, Error> {
        let Expr::MethodCall(call) = base else {
            return Err(Error::new(
                base.span(),
                "a wait point is a call of `s`'s wait points",
            ));
        };
        let argument = |what: &str| {
            call.args.first().cloned().ok_or_else(|| {
                Error::new(
                    call.method.span(),
                    format!("`{}` takes {what}", call.method),
                )
            })
        };
        if call.method == "within" {
            let limit = argument(
                "the longest time the wait may last: `.within(Time::from_millis(s, 2_000)?)`",
            )?;
            let mut wait = self.wait_of(&call.receiver)?;
            wait.within = Some(limit);
            return Ok(wait);
        }
        let is_context =
            matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident(&self.context));
        let is_child = matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident("self"))
            && call.args.len() == 1
            && call.args.first().is_some_and(
                |argument| matches!(argument, Expr::Path(path) if path.path.is_ident(&self.context)),
            );
        let check =
            match call.method.to_string().as_str() {
                "all" if is_context => Check::All(self.members(call)?),
                "race" if is_context => Check::Race(self.members(call)?),
                _ if is_context => self.member(base)?,
                _ if is_child => Check::Child(call.method.clone()),
                _ => return Err(Error::new(
                    base.span(),
                    "a wait point is `s.next_cycle()`, `s.until(..)`, `s.wait(..)`, `s.all(..)`, \
                     `s.race(..)` or a child sequence `self.<child>(s)` (specification 48 RU-72)",
                )),
            };
        Ok(WaitPoint {
            check,
            within: None,
        })
    }

    /// The members of `s.all(..)` or `s.race(..)`, which the reader has checked.
    fn members(&self, call: &syn::ExprMethodCall) -> Result<Vec<Check>, Error> {
        call.args.iter().map(|member| self.member(member)).collect()
    }

    /// A plain wait: `s.next_cycle()`, `s.until(pred)` or `s.wait(t)`.
    fn member(&self, wait: &Expr) -> Result<Check, Error> {
        let not_a_wait = || {
            Error::new(
                wait.span(),
                "a wait is `s.next_cycle()`, `s.until(..)` or `s.wait(..)` (specification 48 RU-72)",
            )
        };
        let Expr::MethodCall(call) = wait else {
            return Err(not_a_wait());
        };
        if !matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident(&self.context)) {
            return Err(not_a_wait());
        }
        let argument = |what: &str| {
            call.args.first().cloned().ok_or_else(|| {
                Error::new(
                    call.method.span(),
                    format!("`{}` takes {what}", call.method),
                )
            })
        };
        match call.method.to_string().as_str() {
            "next_cycle" => Ok(Check::NextCycle),
            "until" => Ok(Check::Until(argument(
                "the predicate: `s.until(|me: &Self| me.ready)`",
            )?)),
            "wait" => Ok(Check::Wait(argument(
                "the time to wait: `s.wait(Time::from_millis(s, 500)?)`",
            )?)),
            _ => Err(not_a_wait()),
        }
    }

    /// The wait point of this number.
    fn wait(&self, number: u16) -> Option<&WaitPoint> {
        usize::from(number)
            .checked_sub(1)
            .and_then(|index| self.waits.get(index))
    }

    /// The child sequence the wait point of this number awaits, when it awaits one.
    pub(super) fn child_of(&self, number: u16) -> Option<&Ident> {
        match self.wait(number) {
            Some(WaitPoint {
                check: Check::Child(child),
                ..
            }) => Some(child),
            _ => None,
        }
    }

    /// Whether the wait point of this number is an `s.all`.
    pub(super) fn is_all(&self, number: u16) -> bool {
        matches!(
            self.wait(number),
            Some(WaitPoint {
                check: Check::All(_),
                ..
            })
        )
    }

    /// The check of the wait point `number` when an activation resumes it: `.within(t)` first,
    /// then whether the wait is over; a wait that is not over returns and waits on.
    pub(super) fn resume(&self, number: u16) -> TokenStream {
        let (context, state, waiting) = (&self.context, &self.state, self.waiting());
        let Some(wait) = self.wait(number) else {
            return TokenStream::new();
        };
        // An argument may use the context itself (`Time::from_millis(s, 500)?`), so it is evaluated
        // before the check borrows the context.
        let within = wait.within.as_ref().map(|limit| {
            quote! {
                let __trust_limit = #limit;
                if let ::core::option::Option::Some(fault) =
                    __trust_wait.outlasted(#context, __trust_limit)
                {
                    return ::core::result::Result::Err(fault);
                }
            }
        });
        let over = match &wait.check {
            Check::All(members) => {
                // Each member not yet complete is checked in order and, once complete, stays so in
                // its bit of `done` (RU-72).
                let checks = members.iter().zip(0_u32..).map(|(member, bit)| {
                    let bit = Literal::u32_unsuffixed(bit);
                    let condition = if let Some(over) = self.over(member) {
                        quote! { !__trust_done.bit(#bit) && (#over) }
                    } else {
                        quote! { !__trust_done.bit(#bit) }
                    };
                    quote! {
                        if #condition {
                            __trust_done = __trust_done.with_bit(#bit, true);
                        }
                    }
                });
                let every = Literal::u32_unsuffixed((1_u32 << members.len()) - 1);
                quote! {
                    let mut __trust_done: ::trust_plc::Word = #state.done;
                    #(#checks)*
                    if u32::from(__trust_done.0) != #every {
                        #state.done = __trust_done;
                        return #waiting;
                    }
                    #state.done = ::trust_plc::Word(0);
                }
            }
            Check::Race(members) => {
                // The members are checked in order until one completes, whose number from 1 is
                // the race's value (RU-72, IEC Table 61 feature 2a); an `s.next_cycle()` always
                // completes, so the members after it are never reached.
                let mut tests = Vec::new();
                let mut numbers = Vec::new();
                let mut last = Literal::u16_unsuffixed(0);
                for (member, number) in members.iter().zip(1_u16..) {
                    let Some(over) = self.over(member) else {
                        last = Literal::u16_unsuffixed(number);
                        break;
                    };
                    tests.push(over);
                    numbers.push(Literal::u16_unsuffixed(number));
                }
                let won = if tests.is_empty() {
                    quote! { #last }
                } else {
                    quote! { #(if #tests { #numbers } else)* { #last } }
                };
                quote! {
                    let __trust_won: u16 = #won;
                    if __trust_won == 0 {
                        return #waiting;
                    }
                }
            }
            // The child runs on from where it waits; the sequence waits on until it has returned.
            Check::Child(child) => quote! {
                if !self.#child(#context)? {
                    return #waiting;
                }
            },
            plain => self
                .over(plain)
                .map(|over| {
                    quote! {
                        if !(#over) {
                            return #waiting;
                        }
                    }
                })
                .unwrap_or_default(),
        };
        quote! {
            let __trust_wait = ::trust_plc::seq::Waiting {
                point: #state.point,
                began: #state.began,
            };
            #within
            #over
        }
    }

    /// Whether the plain wait `check` is over at this activation, one that resumes its wait; `None`
    /// for `s.next_cycle()`, which every such activation completes. An argument may use the
    /// context itself (`Time::from_millis(s, 500)?`), so it is evaluated before the check borrows
    /// the context.
    fn over(&self, check: &Check) -> Option<TokenStream> {
        let context = &self.context;
        match check {
            Check::Until(predicate) => Some(quote! {{
                let __trust_until = #predicate;
                __trust_until(&*self)
            }}),
            Check::Wait(time) => Some(quote! {{
                let __trust_time = #time;
                __trust_wait.waited_for(#context, __trust_time)
            }}),
            // A member is a plain wait: the reader refuses an `s.all`, an `s.race` or a child
            // sequence inside another.
            Check::NextCycle | Check::All(_) | Check::Race(_) | Check::Child(_) => None,
        }
    }
}
