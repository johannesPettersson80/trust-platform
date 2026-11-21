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
            parameters: Vec::new(),
            locals: Vec::new(),
            awaits: Vec::new(),
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
                self.homing(s).await?;
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
    assert_eq!(read.awaits, ["homing"]);
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
            ("shape_ba73d595", &DataType::Elementary(Elementary::Bool)),
            ("count", &DataType::Elementary(Elementary::Int)),
        ]
    );
}

#[test]
fn an_all_and_a_race_read_as_wait_points_and_the_race_gives_its_local() {
    // RU-72: an s.all limited by .within and an s.race whose member number goes into the u16 local
    // `won` are wait points 1 and 2; `won` is a local of the state; and since the body holds an
    // s.all, the state has `done : WORD` after `began`, before the locals (RU-71). Catches a race's
    // value taken for a plain local, a lost .within, and a state laid out apart from the macro's.
    let read = sequence(&parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                s.all(s.until(|me: &Self| me.left), s.wait(Time::from_millis(s, 30)?))
                    .within(Time::from_millis(s, 100)?)
                    .await?;
                let won: u16 = s.race(s.until(|me: &Self| me.part), s.next_cycle()).await;
                self.ended = won;
            }
        }
    })
    .expect("the sequence reads");
    assert_eq!(
        read.wait_points,
        vec![
            point(1, Wait::All, true, None),
            point(2, Wait::Race, false, None),
        ]
    );
    assert_eq!(
        read.locals,
        vec![Local {
            name: "won".into(),
            ty: DataType::Elementary(Elementary::Uint),
        }]
    );
    assert!(read.holds_all());
    let state = crate::sequence::state("PRESS", "Press", &read);
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
            ("done", &DataType::Elementary(Elementary::Word)),
            ("shape_dfd956cf", &DataType::Elementary(Elementary::Bool)),
            ("won", &DataType::Elementary(Elementary::Uint)),
        ]
    );
}

#[test]
fn a_sequence_without_an_all_has_no_done_and_may_name_a_local_so() {
    // RU-71: `done` is a field of the state only when the body holds an s.all, so a sequence
    // without one may have a local `done`, and a loop whose only wait point is a race into a local
    // waits on every way through it. Catches a field every state would carry, and a loop refused
    // although its `let` waits.
    let read = sequence(&parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut done: bool = false;
            loop {
                let won: u16 = s.race(s.until(|me: &Self| me.a), s.until(|me: &Self| me.b)).await;
                done = won == 1;
            }
        }
    })
    .expect("the sequence reads");
    assert!(!read.holds_all());
    let state = crate::sequence::state("CELL", "Cell", &read);
    let names: Vec<&str> = state
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(names, ["point", "began", "shape_350d2ce0", "done", "won"]);
    assert_eq!(state.fields[3].ty, DataType::Elementary(Elementary::Bool));
}

#[test]
fn an_all_or_a_race_with_members_or_values_it_cannot_take_is_refused() {
    // RU-71 and RU-72's refusals for s.all and s.race, each with the words of its message. Catches
    // a mask with more members than bits, a member whose own limit would be lost, a wait inside a
    // wait, a value read from a wait point that gives none, a member number in a type that does not
    // hold it, and locals the state's own fields would hide.
    let many: String = (0..17)
        .map(|_| "s.next_cycle()")
        .collect::<Vec<_>>()
        .join(", ");
    let seventeen: syn::ImplItemFn = syn::parse_str(&format!(
        "async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {{ s.all({many}).await; Ok(()) }}"
    ))
    .expect("a sequence");
    let cases: Vec<(syn::ImplItemFn, &str)> = vec![
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                s.all(s.until(|me: &Self| me.left)).await;
                Ok(())
            } },
            "`s.all` takes two to sixteen waits",
        ),
        (seventeen, "`s.all` takes two to sixteen waits"),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                s.race(
                    s.until(|me: &Self| me.left).within(Time::from_millis(s, 10)?),
                    s.next_cycle(),
                ).await;
                Ok(())
            } },
            "a member of `s.race` takes no `.within` of its own",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                s.all(s.race(s.next_cycle(), s.next_cycle()), s.next_cycle()).await;
                Ok(())
            } },
            "a member of `s.all` is `s.next_cycle()`, `s.until(..)`, `s.wait(..)` or a child sequence",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                s.race(homing(s), s.next_cycle()).await;
                Ok(())
            } },
            "a member of `s.race` is `s.next_cycle()`",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let waited: u16 = s.wait(Time::from_millis(s, 10)?).await;
                Ok(())
            } },
            "only `s.race` gives a value",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let won: i32 = s.race(s.next_cycle(), s.next_cycle()).await;
                Ok(())
            } },
            "is a `u16`",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let point: i16 = 0;
                s.next_cycle().await;
                Ok(())
            } },
            "cannot be named `point`",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let Began: i16 = 0;
                s.next_cycle().await;
                Ok(())
            } },
            "cannot be named `began`",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let Done: bool = false;
                s.all(s.next_cycle(), s.next_cycle()).await;
                Ok(())
            } },
            "cannot be named `done`",
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
fn a_local_is_named_as_a_variable_of_structured_text_is() {
    // RU-71: a local is a field of the state in Structured Text, so it takes an IEC identifier and
    // no reserved word, as a POU's variable does (IEC §6.1.2, specification 01 §3), and a raw
    // identifier is read without its `r#`. Catches a declaration the generated Structured Text
    // could not compile, with the error far from the local.
    let cases: Vec<(syn::ImplItemFn, &str)> = vec![
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let step: i16 = 0;
                s.next_cycle().await;
                Ok(())
            } },
            "`step` is a reserved word of Structured Text",
        ),
        (
            parse_quote! { async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let count_: i16 = 0;
                s.next_cycle().await;
                Ok(())
            } },
            "`count_` is not an IEC identifier",
        ),
    ];
    for (item, reason) in cases {
        let message = refused(&item);
        assert!(
            message.contains(reason),
            "expected `{reason}` in: {message}"
        );
    }
    let read = sequence(&parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let r#ready: bool = false;
            s.next_cycle().await;
            Ok(())
        }
    })
    .expect("the sequence reads");
    assert_eq!(read.locals[0].name, "ready");
}

/// The filler's `#[trust_sequence]` block: the POU's sequence `run`, its child `fill`, which awaits
/// the child `settle`, declared before `fill` so that the block's order is not the order of awaits.
fn filler() -> syn::ItemImpl {
    parse_quote! {
        #[trust_sequence]
        impl Filler {
            async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                loop {
                    s.until(|me: &Self| me.bottle).await;
                    self.fill(s).within(Time::from_millis(s, 100)?).await?;
                    s.next_cycle().await;
                }
            }

            async fn settle(&mut self, s: &mut Seq) -> Result<(), Fault> {
                let mut ticks: i16 = 0;
                while ticks < 2 {
                    ticks = checked::add(s, ticks, 1)?;
                    s.next_cycle().await;
                }
                Ok(())
            }

            async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> {
                self.valve = true;
                s.until(|me: &Self| me.full).await;
                self.valve = false;
                self.settle(s).await?;
                Ok(())
            }
        }
    }
}

/// A state's structure as its name, its Rust type and its fields with their types.
type LaidOut<'a> = (&'a str, &'a str, Vec<(&'a str, &'a DataType)>);

#[test]
fn a_block_holds_the_pous_sequence_and_its_children_in_the_blocks_order() {
    // RU-71 to RU-73: the POU's sequence is the one no other awaits, `run`; the children follow in
    // the block's order, `settle` before `fill`. The POU's state holds the block's shape,
    // shape_eb6a7681, computed independently from run's lines, `child settle` and settle's lines,
    // `child fill` and fill's lines, and a field for each child after its locals, of the child's own
    // state, `FILLER_SEQ_SETTLE` and `FILLER_SEQ_FILL`, laid out as a sequence's state without a
    // shape. Catches a child taken for the POU's sequence, children in the order of their awaits, a
    // child's state the POU's state would not hold, and a shape that a child's change would keep.
    let block = crate::sequence::block(&filler()).expect("the block reads");
    assert_eq!(block.owner, "Filler");
    assert_eq!(block.sequence.name, "run");
    assert_eq!(block.sequence.awaits, ["fill"]);
    let children: Vec<(&str, &[String])> = block
        .children
        .iter()
        .map(|child| (child.name.as_str(), child.awaits.as_slice()))
        .collect();
    assert_eq!(
        children,
        [("settle", &[][..]), ("fill", &["settle".to_string()][..])]
    );
    let states = crate::sequence::states("FILLER", "Filler", &block);
    let laid_out: Vec<LaidOut<'_>> = states
        .iter()
        .map(|state| {
            (
                state.name.as_str(),
                state.item.as_str(),
                state
                    .fields
                    .iter()
                    .map(|field| (field.name.as_str(), &field.ty))
                    .collect(),
            )
        })
        .collect();
    let uint = DataType::Elementary(Elementary::Uint);
    let time = DataType::Elementary(Elementary::Time);
    let int = DataType::Elementary(Elementary::Int);
    let boolean = DataType::Elementary(Elementary::Bool);
    let settle = DataType::Named("FillerSequenceSettle".into());
    let fill = DataType::Named("FillerSequenceFill".into());
    assert_eq!(
        laid_out,
        [
            (
                "FILLER_SEQ",
                "FillerSequence",
                vec![
                    ("point", &uint),
                    ("began", &time),
                    ("shape_eb6a7681", &boolean),
                    ("settle", &settle),
                    ("fill", &fill)
                ]
            ),
            (
                "FILLER_SEQ_SETTLE",
                "FillerSequenceSettle",
                vec![("point", &uint), ("began", &time), ("ticks", &int)]
            ),
            (
                "FILLER_SEQ_FILL",
                "FillerSequenceFill",
                vec![("point", &uint), ("began", &time)]
            ),
        ]
    );
    assert_eq!(
        crate::sequence::child_state_item("Palletizer", "fill_box"),
        "PalletizerSequenceFillBox"
    );
}

#[test]
fn a_block_whose_sequences_cannot_run_as_one_is_refused() {
    // RU-72's refusals that need the whole block, and the receiver and parameters of a sequence,
    // each with the words of its message. Catches a sequence whose state two activations would
    // share (one that awaits itself), a child that does not exist or whose fault would be lost, a
    // POU with two sequences, and a sequence that cannot change its POU.
    let cases: Vec<(syn::ItemImpl, &str)> = vec![
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                    loop { s.next_cycle().await; self.run(s).await?; }
                }
            } },
            "`run` awaits itself through `run`",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.a(s).await?; Ok(()) }
                async fn a(&mut self, s: &mut Seq) -> Result<(), Fault> { self.b(s).await?; Ok(()) }
                async fn b(&mut self, s: &mut Seq) -> Result<(), Fault> { self.a(s).await?; Ok(()) }
            } },
            "`a` awaits itself through `b`",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                    self.missing(s).await?;
                    Ok(())
                }
            } },
            "`self.missing(..)` awaits no sequence of this block",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
                async fn orphan(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "`run` and `orphan` are both awaited by no sequence of the block",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.fill(s).await?; Ok(()) }
                async fn fill(&mut self, s: &mut Seq, n: i16) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "passes 0 argument(s) after the context to the child sequence `fill`, which takes 1",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "a sequence is a method of its POU that changes it",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.fill(s).await; Ok(()) }
                async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "a child sequence's fault is the fault of the sequence that awaits it",
        ),
        (
            parse_quote! { impl Sequenced for Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "not a trait's",
        ),
    ];
    refused_blocks(cases);
}

/// Each block of `cases` refused with its reason's words in the message.
fn refused_blocks(cases: Vec<(syn::ItemImpl, &str)>) {
    for (item, reason) in cases {
        let message = crate::sequence::block(&item)
            .expect_err("the block is refused")
            .to_string();
        assert!(
            message.contains(reason),
            "expected `{reason}` in: {message}"
        );
    }
}

#[test]
fn a_child_named_as_no_field_of_the_state_can_be_is_refused() {
    // RU-71: a child keeps its state in the field `_seq.<child>` of the structure
    // `<POU>_SEQ_<CHILD>`, so its name is an IEC identifier that is no reserved word, begins with a
    // letter, and names no field the state has already, in any case. Catches a state whose field
    // would hide a local or the wait point, and a declaration Structured Text could not compile.
    let cases: Vec<(syn::ItemImpl, &str)> = vec![
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                    let fill: i16 = 0;
                    self.fill(s).await?;
                    Ok(())
                }
                async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "which the state of `run` has already as `fill`",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.Point(s).await?; Ok(()) }
                async fn Point(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "has already as `point`",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self._fill(s).await?; Ok(()) }
                async fn _fill(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "a child sequence's name begins with a letter",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.step(s).await?; Ok(()) }
                async fn step(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "`step` is a reserved word of Structured Text",
        ),
    ];
    refused_blocks(cases);
}

#[test]
fn a_loop_waits_only_where_every_way_round_waits() {
    // RU-72's loop rule over the whole block: a child sequence that can return without waiting is
    // no wait point, so a loop whose only wait is awaiting it could spin a scan; an inner loop that
    // can `break` before its wait lets the outer one spin; a labelled block's `break` would skip its
    // wait. A loop that awaits a child that always waits, and one that breaks only after waiting,
    // are taken. Catches a scan that never ends behind a child or an inner loop, and a rule that
    // refuses loops that do wait.
    let skips: syn::ItemImpl = parse_quote! { impl Cell {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop { self.maybe(s).await?; }
        }
        async fn maybe(&mut self, s: &mut Seq) -> Result<(), Fault> {
            if self.skip {
                return Ok(());
            }
            s.next_cycle().await;
            Ok(())
        }
    } };
    let message = crate::sequence::block(&skips)
        .expect_err("the loop could spin")
        .to_string();
    assert!(
        message.contains("a child sequence it awaits can return without waiting"),
        "{message}"
    );
    let inner = refused(&parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                loop {
                    if self.ready {
                        break;
                    }
                    s.next_cycle().await;
                }
            }
        }
    });
    assert!(
        inner.contains("can run without reaching a wait point"),
        "{inner}"
    );
    let labelled = refused(&parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            'wait: {
                if self.ready {
                    break 'wait;
                }
                s.next_cycle().await;
            }
            Ok(())
        }
    });
    assert!(
        labelled.contains("a labelled block cannot hold a wait point"),
        "{labelled}"
    );
    let waits: syn::ItemImpl = parse_quote! { impl Cell {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                self.fill(s).await?;
                loop {
                    s.next_cycle().await;
                    if self.ready {
                        break;
                    }
                }
            }
        }
        async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> {
            s.until(|me: &Self| me.full).await;
            Ok(())
        }
    } };
    assert!(crate::sequence::block(&waits).is_ok());
}

#[test]
fn a_sequences_shape_changes_with_its_wait_points_and_locals_and_with_nothing_else() {
    // RU-73: the shape is `shape_` and the FNV-1a hash of the lines `wait WAIT_PART`, `wait GRIP`
    // and `wait #3`, a wait point named by its step or by its number, then `local parts: INT`;
    // 0xd61125c5 was computed independently from those lines. Plain code that changes, a step's
    // wait that waits otherwise, and a local renamed only in case keep it; a wait point added before
    // the others, a step renamed, and a local of another type change it. Catches a shape that a
    // change of plain code would reset, and one that keeps a state whose wait points moved.
    let shape = |body: syn::ImplItemFn| crate::sequence::shape(&sequence(&body).expect("reads"));
    let running = shape(parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut parts: i16 = 0;
            loop {
                s.step(&mut self.active_step, "WAIT_PART");
                s.until(|me: &Self| me.part_present).await;
                s.step(&mut self.active_step, "GRIP");
                self.close_gripper = true;
                s.until(|me: &Self| me.gripper_closed).await;
                s.wait(Time::from_millis(s, 500)?).await;
                parts = checked::add(s, parts, 1)?;
            }
        }
    });
    assert_eq!(running, "shape_d61125c5");
    let kept = shape(parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut Parts: i16 = 0;
            loop {
                s.step(&mut self.active_step, "WAIT_PART");
                s.until(|me: &Self| me.part_present && me.enabled).await;
                s.step(&mut self.active_step, "GRIP");
                self.close_gripper = self.enabled;
                s.wait(Time::from_millis(s, 300)?).await;
                s.next_cycle().await;
                Parts = checked::add(s, Parts, 2)?;
            }
        }
    });
    assert_eq!(kept, running);
    let added = shape(parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut parts: i16 = 0;
            loop {
                s.next_cycle().await;
                s.step(&mut self.active_step, "WAIT_PART");
                s.until(|me: &Self| me.part_present).await;
                s.step(&mut self.active_step, "GRIP");
                s.until(|me: &Self| me.gripper_closed).await;
                s.wait(Time::from_millis(s, 500)?).await;
            }
        }
    });
    let renamed = shape(parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut parts: i16 = 0;
            loop {
                s.step(&mut self.active_step, "WAIT_PART");
                s.until(|me: &Self| me.part_present).await;
                s.step(&mut self.active_step, "CLAMP");
                s.until(|me: &Self| me.gripper_closed).await;
                s.wait(Time::from_millis(s, 500)?).await;
            }
        }
    });
    let retyped = shape(parse_quote! {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            let mut parts: i32 = 0;
            loop {
                s.step(&mut self.active_step, "WAIT_PART");
                s.until(|me: &Self| me.part_present).await;
                s.step(&mut self.active_step, "GRIP");
                s.until(|me: &Self| me.gripper_closed).await;
                s.wait(Time::from_millis(s, 500)?).await;
            }
        }
    });
    for changed in [added, renamed, retyped] {
        assert_ne!(changed, running);
    }
}

#[test]
fn a_childs_parameters_are_variables_of_its_state_before_its_locals() {
    // RU-71 and RU-72: the child `go` takes `goal: f64` after its context; the parameter is a
    // variable of its state before the local `tries`, and the child's state has no shape, the POU's
    // state's alone (RU-73). The POU's sequence passes the one argument. Catches a parameter left out
    // of the state, which the wait would lose.
    let block = crate::sequence::block(&parse_quote! { impl Shuttle {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                self.go(s, 10.0).await?;
                self.go(s, 0.0).await?;
            }
        }
        async fn go(&mut self, s: &mut Seq, goal: f64) -> Result<(), Fault> {
            let mut tries: i16 = 0;
            self.target = goal;
            s.until(|me: &Self| me.arrived).await;
            tries = 1;
            Ok(())
        }
    } })
    .expect("the block reads");
    let go = &block.children[0];
    assert_eq!(
        go.parameters,
        [Local {
            name: "goal".into(),
            ty: DataType::Elementary(Elementary::Lreal),
        }]
    );
    let states = crate::sequence::states("SHUTTLE", "Shuttle", &block);
    let names: Vec<&str> = states[1]
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(names, ["point", "began", "goal", "tries"]);
}

#[test]
fn a_parameter_or_an_argument_the_child_cannot_take_is_refused() {
    // RU-72's refusals for a child's parameters, each with the words of its message. Catches a POU
    // sequence its activation could not call, an argument with no parameter to set, a parameter no
    // state field can hold, and a local the parameter would hide.
    let cases: Vec<(syn::ItemImpl, &str)> = vec![
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq, speed: f64) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "the POU's sequence `run` takes no parameter but its context",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.go(s).await?; Ok(()) }
                async fn go(&mut self, s: &mut Seq, goal: f64) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "passes 0 argument(s) after the context to the child sequence `go`, which takes 1",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.go(s, 1.0).await?; Ok(()) }
                async fn go(&mut self, s: &mut Seq, (a, b): (f64, f64)) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "a child sequence's parameter is one name with its type",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.go(s, 1).await?; Ok(()) }
                async fn go(&mut self, s: &mut Seq, Point: u16) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
            } },
            "cannot be named `point`",
        ),
        (
            parse_quote! { impl Cell {
                async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { self.go(s, 1.0).await?; Ok(()) }
                async fn go(&mut self, s: &mut Seq, goal: f64) -> Result<(), Fault> {
                    let Goal: f64 = 0.0;
                    s.next_cycle().await;
                    Ok(())
                }
            } },
            "a local `Goal` beside the parameter `goal`",
        ),
    ];
    refused_blocks(cases);
}

#[test]
fn an_all_or_a_race_takes_child_sequences_as_members() {
    // RU-72: `s.all(self.left(s), self.right(s))` awaits both children, which the block reads as
    // children of the POU's sequence, one state each; a child twice in one s.race is refused, as it
    // has one state. Catches a child member the block would not declare a state for, and two members
    // that would share one.
    let block = crate::sequence::block(&parse_quote! { impl Twin {
        async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
            loop {
                s.all(self.left(s), self.right(s)).await;
                s.next_cycle().await;
            }
        }
        async fn left(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
        async fn right(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
    } })
    .expect("the block reads");
    assert_eq!(block.sequence.awaits, ["left", "right"]);
    let names: Vec<&str> = block
        .children
        .iter()
        .map(|child| child.name.as_str())
        .collect();
    assert_eq!(names, ["left", "right"]);
    refused_blocks(vec![(
        parse_quote! { impl Cell {
            async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
                s.race(self.fill(s), self.fill(s)).await;
                Ok(())
            }
            async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> { s.next_cycle().await; Ok(()) }
        } },
        "the child sequence `fill` is a member of this `s.race` twice",
    )]);
}

#[test]
fn a_loop_counts_an_all_or_a_race_of_children_as_a_wait_only_when_it_waits() {
    // RU-72's loop rule with child members: an s.race completes at once when one child member
    // returns at once, and an s.all when all its members do, so a loop whose only wait is such a
    // wait point could spin a scan; an s.all with a plain member always waits. `quick` and `brief`
    // return at once when the cell is ready, `slow` always waits. Catches a loop that spins behind a
    // race or an all of children, and a rule that refuses an s.all that does wait.
    let quick = "async fn quick(&mut self, s: &mut Seq) -> Result<(), Fault> { \
                 if self.ready { return Ok(()); } s.next_cycle().await; Ok(()) }";
    let brief = "async fn brief(&mut self, s: &mut Seq) -> Result<(), Fault> { \
                 if self.ready { return Ok(()); } s.next_cycle().await; Ok(()) }";
    let slow = "async fn slow(&mut self, s: &mut Seq) -> Result<(), Fault> { \
                s.next_cycle().await; Ok(()) }";
    let block = |run: &str, children: &[&str]| -> syn::ItemImpl {
        syn::parse_str(&format!("impl Cell {{ {run} {} }}", children.join(" "))).expect("a block")
    };
    for (run, children) in [
        (
            "async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { \
             loop { s.race(self.slow(s), self.quick(s)).await; } }",
            [slow, quick],
        ),
        (
            "async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { \
             loop { s.all(self.quick(s), self.brief(s)).await; } }",
            [quick, brief],
        ),
    ] {
        let message = crate::sequence::block(&block(run, &children))
            .expect_err("the wait point could complete at once")
            .to_string();
        assert!(message.contains("can return without waiting"), "{message}");
    }
    let waits = "async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> { \
                 loop { s.all(self.quick(s), s.next_cycle()).await; } }";
    crate::sequence::block(&block(waits, &[quick])).expect("the s.all waits on its plain member");
}
