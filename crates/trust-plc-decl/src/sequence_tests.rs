//! A `#[trust_sequence]`'s body as RU-71 and RU-72 read it: its wait points with their steps, its
//! locals, and each refusal at its place.

use syn::parse_quote;

use crate::sequence::{sequence, Local, Sequence, Wait, WaitPoint};
use crate::types::{DataType, Elementary};

fn point(number: u16, wait: Wait, within: bool, step: Option<&str>) -> WaitPoint {
    WaitPoint {
        number,
        wait,
        within,
        step: step.map(str::to_owned),
    }
}

/// The refusal of `item`'s sequence, as its message.
fn refused(item: &syn::ImplItemFn) -> String {
    sequence(item)
        .expect_err("the sequence is refused")
        .to_string()
}

#[test]
fn the_palletizer_has_three_wait_points_named_by_their_steps() {
    // RU-70's example: until a part is present in the step WAIT_PART, until the gripper has closed
    // within 2 s in the step GRIP, then 500 ms: three wait points numbered in the body's order, the
    // first two named by the s.step before them, the third by none since GRIP's already named the
    // second. Catches wait points counted from 0, a .within lost, and a step name kept for two.
    let item: syn::ImplItemFn = parse_quote! {
        #[trust_sequence]
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                s.step(&mut self.active_step, "WAIT_PART");
                s.until(|me: &Self| me.part_present).await;
                s.step(&mut self.active_step, "GRIP");
                self.close_gripper = true;
                s.until(|me: &Self| me.gripper_closed)
                    .within(Time::from_secs(2))
                    .await?;
                s.wait(Time::from_millis(500)).await;
                self.close_gripper = false;
            }
        }
    };
    assert_eq!(
        sequence(&item).expect("the sequence reads"),
        Sequence {
            name: "run".into(),
            wait_points: vec![
                point(1, Wait::Until, false, Some("WAIT_PART")),
                point(2, Wait::Until, true, Some("GRIP")),
                point(3, Wait::Wait, false, None),
            ],
            locals: Vec::new(),
        }
    );
}

#[test]
fn a_local_is_a_variable_of_the_state_with_its_type() {
    // RU-71: a typed local becomes a variable of the sequence's state, with its IEC type; the wait
    // points inside while and if count in the body's order, next_cycle and a child sequence
    // among them. Catches a local lost, or one read with another type.
    let item: syn::ImplItemFn = parse_quote! {
        #[trust_sequence]
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut count: i16 = 0;
            while count < 3 {
                count += 1;
                s.next_cycle().await;
            }
            if self.home {
                self.homing(s).await;
            } else {
                s.race(s.wait(Time::from_secs(1)), s.until(|me: &Self| me.stop)).await;
            }
            Ok(())
        }
    };
    let read = sequence(&item).expect("the sequence reads");
    assert_eq!(
        read.locals,
        vec![Local {
            name: "count".into(),
            ty: DataType::Elementary(Elementary::Int),
        }]
    );
    assert_eq!(
        read.wait_points,
        vec![
            point(1, Wait::NextCycle, false, None),
            point(2, Wait::Child, false, None),
            point(3, Wait::Race, false, None),
        ]
    );
}

#[test]
fn every_refusal_names_its_place_and_reason() {
    // RU-72's refusals, one each, with the words of their message. Catches a body the attribute
    // would lower into a state machine that spins a scan forever, loses a value across a wait, or
    // keeps a future.
    let cases: Vec<(syn::ImplItemFn, &str)> = vec![
        (
            parse_quote! { fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { Ok(()) } },
            "a sequence is an `async fn",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let count = 0;
                s.next_cycle().await;
                Ok(())
            } },
            "names its type",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let count: i16 = 0;
                let Count: i16 = 1;
                s.next_cycle().await;
                Ok(())
            } },
            "a second local `Count`",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                fetch().await;
                Ok(())
            } },
            "awaits only its wait points",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                s.sleep(Time::from_secs(1)).await;
                Ok(())
            } },
            "`s.sleep` is no wait point",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let ready: bool = s.until(|me: &Self| me.ready).await;
                Ok(())
            } },
            "stands as a statement of its own",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let later = async { 1 };
                Ok(())
            } },
            "an `async` block",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                loop {
                    self.count += 1;
                }
            } },
            "can run without reaching a wait point",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                loop {
                    if self.skip {
                        continue;
                    }
                    s.next_cycle().await;
                }
            } },
            "can run without reaching a wait point",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                loop {
                    if self.fast {
                        s.next_cycle().await;
                    }
                }
            } },
            "can run without reaching a wait point",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                for _ in 0..3 {
                    s.next_cycle().await;
                }
                Ok(())
            } },
            "a `for` loop cannot hold a wait point",
        ),
    ];
    for (item, reason) in cases {
        let message = refused(&item);
        assert!(
            message.contains(reason),
            "expected `{reason}` in: {message}"
        );
    }
}

#[test]
fn a_loop_that_waits_on_every_way_through_is_taken() {
    // A loop whose if waits on both branches, and one whose continue comes after its wait, reach a
    // wait point on every way through. Catches a check that refuses a loop that does wait.
    let item: syn::ImplItemFn = parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                if self.fast {
                    s.next_cycle().await;
                } else {
                    s.wait(Time::from_millis(100)).await;
                }
                s.next_cycle().await;
                if self.again {
                    continue;
                }
            }
        }
    };
    assert_eq!(
        sequence(&item).expect("the loop waits").wait_points.len(),
        3
    );
}

/// A POU's struct with `fields`, whose attribute has the arguments `args`.
fn pou_struct(fields: &str) -> syn::ItemStruct {
    syn::parse_str(&format!(
        "#[budget(\"1us\")] pub struct Cell {{ {fields} }}"
    ))
    .expect("a struct")
}

fn arguments(text: &str) -> proc_macro2::TokenStream {
    text.parse().expect("the arguments are tokens")
}

#[test]
fn a_block_or_a_program_names_the_sequence_its_body_runs() {
    // RU-71: `sequence = "run"` names the `async fn` of the POU's `#[trust_sequence]` block, by its
    // Rust name, on a function block as on a program; a POU without it runs its own `impl Body`.
    // Catches a reader that drops the name, so that the attribute would give the POU no state.
    let item = pou_struct("#[input] pub start: bool");
    let block = crate::function_block(arguments("name = \"CELL\", sequence = \"run\""), &item)
        .expect("the block reads");
    assert_eq!(block.sequence.as_deref(), Some("run"));
    let program = crate::program(arguments("name = \"CELL\", sequence = \"cycle\""), &item)
        .expect("the program reads");
    assert_eq!(program.sequence.as_deref(), Some("cycle"));
    let plain = crate::program(arguments("name = \"CELL\""), &item).expect("the program reads");
    assert_eq!(plain.sequence, None);
}

#[test]
fn a_sequence_that_is_no_rust_name_or_a_variable_named_as_the_state_is_refused() {
    // RU-71 and RU-72: `sequence` takes a Rust name; a POU that runs a sequence may not have a
    // variable `_seq` in any case, its state's name; a POU without one may. `implements` stays a
    // block's. Catches a state that a variable of the user's would hide.
    let item = pou_struct("#[input] pub start: bool");
    let refusal = crate::program(arguments("name = \"CELL\", sequence = \"run it\""), &item)
        .expect_err("not a name")
        .to_string();
    assert!(refusal.contains("`sequence = \"run\"`"), "{refusal}");
    for field in ["_seq", "_SEQ"] {
        let item = pou_struct(&format!("#[input] pub start: bool, {field}: i16"));
        let refusal =
            crate::function_block(arguments("name = \"CELL\", sequence = \"run\""), &item)
                .expect_err("the state's name")
                .to_string();
        assert!(refusal.contains("its variable `_seq`"), "{refusal}");
    }
    let item = pou_struct("#[input] pub start: bool, _seq: i16");
    assert!(crate::function_block(arguments("name = \"CELL\""), &item).is_ok());
    let refusal = crate::program(arguments("name = \"CELL\", implements = \"I_RUN\""), &item)
        .expect_err("a program's interfaces")
        .to_string();
    assert!(refusal.contains("belongs to `#[trust_fb]`"), "{refusal}");
}

#[test]
fn a_pou_that_runs_a_sequence_has_its_state_last_with_the_sequences_locals() {
    // RU-71: the readers add `_seq` as the POU's last variable, an internal one of the type
    // `<Item>Sequence`, and the state's structure `<POU>_SEQ`, in the POU's namespace, holds
    // `point : UINT`, `began : TIME` and the locals in the body's order. Catches a state that the
    // macro and the declarations' writer would lay out differently.
    let item = pou_struct("#[input] pub start: bool");
    let block = crate::function_block(
        arguments("name = \"Motion.CELL\", sequence = \"run\""),
        &item,
    )
    .expect("the block reads");
    let last = block.variables.last().expect("a variable");
    assert_eq!(
        (last.name.as_str(), &last.ty, &last.section, last.retain),
        (
            "_seq",
            &DataType::Named("CellSequence".into()),
            &crate::Section::Var,
            false
        )
    );
    let read = sequence(&parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut count: i16 = 0;
            loop {
                count = 1;
                s.next_cycle().await;
            }
        }
    })
    .expect("the sequence reads");
    let state = crate::sequence::state(&block.name, &block.item, &read);
    assert_eq!(
        (state.name.as_str(), state.item.as_str()),
        ("Motion.CELL_SEQ", "CellSequence")
    );
    let fields: Vec<(&str, &DataType)> = state
        .fields
        .iter()
        .map(|field| (field.name.as_str(), &field.ty))
        .collect();
    assert_eq!(
        fields,
        [
            ("point", &DataType::Elementary(Elementary::Uint)),
            ("began", &DataType::Elementary(Elementary::Time)),
            ("count", &DataType::Elementary(Elementary::Int)),
        ]
    );
}
