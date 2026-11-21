//! `#[trust_sequence]`'s refusals beyond the reader's (specification 48 RU-72): a binding that would
//! have to live across a wait, and a `break` with a value out of a loop that waits. What the
//! lowering runs is proven by `trust-plc`'s tests, which compile and run sequences.

use quote::quote;

use super::expand;

/// The expansion of a `#[trust_sequence]` block whose sequence has the body `body`.
fn expanded(body: &proc_macro2::TokenStream) -> String {
    expand(
        &quote!(),
        &quote! {
            impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                    #body
                }
            }
        },
    )
    .to_string()
}

#[test]
fn an_if_let_or_a_while_let_that_holds_a_wait_point_is_refused() {
    // Their binding would have to live across the wait, which no state of the sequence holds.
    // Catches a lowering that resumes after the wait with the binding gone.
    let if_let = expanded(&quote! {
        loop {
            if let Some(next) = self.next {
                s.next_cycle().await;
                self.last = next;
            }
            s.next_cycle().await;
        }
    });
    assert!(
        if_let.contains("compile_error") && if_let.contains("an `if let` cannot hold a wait point"),
        "{if_let}"
    );
    let while_let = expanded(&quote! {
        while let Some(next) = self.next {
            self.last = next;
            s.next_cycle().await;
        }
        Ok(())
    });
    assert!(
        while_let.contains("compile_error")
            && while_let.contains("a `while let` cannot hold a wait point"),
        "{while_let}"
    );
}

#[test]
fn a_break_with_a_value_out_of_a_loop_that_waits_is_refused() {
    // A loop that holds a wait point is a statement, whose value nothing takes. Catches a lowering
    // that drops the value without a word.
    let found = expanded(&quote! {
        loop {
            s.next_cycle().await;
            if self.done {
                break 5;
            }
        }
    });
    assert!(found.contains("its `break` takes no value"), "{found}");
}
