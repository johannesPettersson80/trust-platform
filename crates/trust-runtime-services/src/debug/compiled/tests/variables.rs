//! The variables a compiled program's handles list (specification 44 §13.2): a frame's locals,
//! the globals, the retained variables, the instances, an instance's members and an array of
//! instances, over the fixture of the parent module.

use super::*;

#[test]
fn the_locals_of_a_program_frame_are_its_programs_variables_in_symbol_order() {
    // Specification 44 §13.2: a frame's locals are the variables of the instance it runs on, then
    // its frame variables. `Main` runs on `Plant`, a program root, so its locals are the root's
    // child symbols in the symbol table's order, each by its own name, the retained `Hits` from the
    // retain area included; `Main` has no frame variables. Worked: `Ctr` a `Counter` of two
    // members (handle 5), `Ctrs` an array of two instances (handle 6), `Hits` 9 (R 4) and `Speed` 5
    // (P 8).
    // Wrong behaviour caught: the program type's members only (no `Hits`, which lies outside the
    // instance), the order of declaration (`Speed` before `Ctrs`), names with the instance's path
    // (`Plant.Speed`), and an instance or an array of instances shown as a value.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);

    assert_eq!(
        held.listed(1),
        json!([
            instance_entry("Ctr", "Counter", 5, 2),
            array_entry("Ctrs", 6, 2),
            scalar("Hits", "9", "DINT"),
            scalar("Speed", "5", "INT"),
        ])
    );
}

#[test]
fn the_locals_of_a_function_frame_are_its_frame_variables_by_their_own_names() {
    // Specification 44 §5.7 and §13.2: a function runs on no instance; its locals are its frame
    // variables, the child symbols of its POU symbol, read from its frame at their offsets and
    // named by their display names after the last `.`; an instance among them is browsed in the
    // frame. Worked: `N` 12 at offset 0, and `Tally`, a `Counter` at offset 4 (handle 5), whose
    // `Step` is 6 at 4 and `Count` 60 at 8 of the frame.
    // Wrong behaviour caught: a frame variable read from an area at its frame offset (area 7 is
    // empty here, so it would not read), a name with the POU's (`SumTo.N`), the members of `Tally`
    // listed as the frame's own, the variables of the calling frame, and an instance in a frame
    // read from the instance area (`Step` would be 40 and `Count` 5).
    let held = Held::new(vec![sum_to(2), main_body(1)]);
    held.open_scopes(2);

    assert_eq!(
        held.listed(1),
        json!([
            scalar("N", "12", "INT"),
            instance_entry("Tally", "Counter", 5, 2)
        ])
    );
    assert_eq!(
        held.listed(5),
        json!([scalar("Step", "6", "INT"), scalar("Count", "60", "DINT")])
    );
}

#[test]
fn the_locals_of_a_block_frame_are_its_instances_members_where_its_program_also_lies() {
    // Specification 44 §13.2: the body of a function block runs on its instance, and its locals are
    // the members of its POU symbol's type at the instance's place. `Ctr` is `Main`'s first
    // variable, so it lies where `Plant` does, at P 0; the program root there is of another type,
    // and the locals are `Counter`'s members. Worked: `Step` 2 at P 0 and `Count` 40 at P 4.
    // Wrong behaviour caught: the program's variables (`Ctr`, `Ctrs`, `Hits`, `Speed`) listed for a
    // block whose instance lies where its program does.
    let held = Held::new(vec![counter_body(3), main_body(0)]);
    held.open_scopes(2);

    assert_eq!(
        held.listed(1),
        json!([scalar("Step", "2", "INT"), scalar("Count", "40", "DINT")])
    );
}

#[test]
fn the_globals_are_the_variables_without_a_parent_in_symbol_order() {
    // Specification 44 §5.7 and §13.2: `Globals` lists the symbols without a parent in the areas of
    // variables, in the symbol table's order and by display name, and no program root, POU symbol,
    // member or frame variable. A value is projected as the interpreter projects it: an array and a
    // structure take handles and report their counts. Worked: `Levels` (G 6, three elements,
    // handle 5), `Limit` 7 (G 0), `Origin` (G 2, a `Point` of two members, handle 6) and `Total`
    // 100 (R 0).
    // Wrong behaviour caught: `Plant`, `@MAIN` or `SumTo.N` listed as globals, `Origin.X` listed
    // beside its structure, the order of declaration, and a retained global left out.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let origin = json!({
        "name": "Origin",
        "value": "Point {...}",
        "type": "Point",
        "variablesReference": 6,
        "evaluateName": "Origin",
        "namedVariables": 2
    });

    assert_eq!(
        held.listed(2),
        json!([
            array_entry("Levels", 5, 3),
            scalar("Limit", "7", "INT"),
            origin,
            scalar("Total", "100", "DINT"),
        ])
    );
}

#[test]
fn the_retained_variables_are_listed_under_their_keys_in_the_maps_order() {
    // Specification 44 §5.10 and §13.2: `Retain` lists the retain map's records in its order, each
    // named by its checkpoint key and read at its area and offset with its type. Worked: `Total`
    // 100 at R 0, then `@program/Plant/Hits` 9 at R 4.
    // Wrong behaviour caught: a variable named by its symbol (`Plant.Hits`), the symbol table's
    // order (`Hits` before `Total`), and a value read from another area.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);

    assert_eq!(
        held.listed(3),
        json!([
            scalar("Total", "100", "DINT"),
            scalar("@program/Plant/Hits", "9", "DINT")
        ])
    );
}

#[test]
fn the_instances_are_the_program_roots_and_block_instances_by_symbol_index() {
    // Specification 44 §13.2: `Instances` lists the program roots and the function block and class
    // instances that lie in an area, ascending by symbol index, each named `<type>#<index>` with
    // its type as its value and type, as the interpreter's instances; the index is the identity
    // that Live Values' `instance:<id>` takes. Worked: `Plant` (symbol 12, type `Main`) and
    // `Plant.Ctr` (symbol 13, `Counter`), handles 5 and 6; `SumTo.Tally` lies in a frame, and the
    // array `Plant.Ctrs` is no instance.
    // Wrong behaviour caught: an instance named by its variable (`Plant`), an identity other than
    // its symbol's index, an instance in a frame listed as if it lay in an area, and an array of
    // instances listed as one.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);

    assert_eq!(
        held.listed(4),
        json!([
            {"name": "Main#12", "value": "Main", "type": "Main", "variablesReference": 5},
            {"name": "Counter#13", "value": "Counter", "type": "Counter", "variablesReference": 6},
        ])
    );
}

#[test]
fn an_instance_lists_its_members_and_a_program_the_variables_of_its_root() {
    // Specification 44 §13.2: an instance's handle lists the variables of the program root of its
    // type that lies at its place, or else its type's members at its place. Worked: `Main#12`
    // (handle 5) lists `Ctr`, `Ctrs`, `Hits` and `Speed`, the first two with handles 7 and 8;
    // `Counter#13` (handle 6), which lies where `Plant` does, lists `Step` 2 and `Count` 40.
    // Wrong behaviour caught: a program listed by its type's members (no `Hits`), and a block that
    // lies where its program does listed as the program.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let _ = held.listed(4);

    assert_eq!(
        held.listed(5),
        json!([
            instance_entry("Ctr", "Counter", 7, 2),
            array_entry("Ctrs", 8, 2),
            scalar("Hits", "9", "DINT"),
            scalar("Speed", "5", "INT"),
        ])
    );
    assert_eq!(
        held.listed(6),
        json!([scalar("Step", "2", "INT"), scalar("Count", "40", "DINT")])
    );
}

#[test]
fn an_array_of_instances_lists_its_elements_as_instances() {
    // Specification 44 §13.2: an element of an array of instances is reached through its array,
    // which lists its elements in row-major order, each an instance one stride further on, named
    // `[k]` by its zero-based index as an array's elements are. Worked: `Ctrs` (handle 6 of
    // `Main`'s locals), two `Counter`s 8 bytes apart from P 12: `[0]` (handle 7) has `Step` 1 and
    // `Count` 10, `[1]` (handle 8) `Step` 3 and `Count` 30.
    // Wrong behaviour caught: elements named by their declared bounds (`[1]` and `[2]`), every
    // element read at the array's start, and an element shown as a value.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);
    let _ = held.listed(1);

    assert_eq!(
        held.listed(6),
        json!([
            instance_entry("[0]", "Counter", 7, 2),
            instance_entry("[1]", "Counter", 8, 2)
        ])
    );
    assert_eq!(
        held.listed(7),
        json!([scalar("Step", "1", "INT"), scalar("Count", "10", "DINT")])
    );
    assert_eq!(
        held.listed(8),
        json!([scalar("Step", "3", "INT"), scalar("Count", "30", "DINT")])
    );
}

#[test]
fn a_value_that_does_not_read_shows_why_without_a_type_or_a_handle() {
    // Specification 44 §13.2 with specification 13's projection: a variable whose value does not
    // read is listed with the error's text as its value, without a type or a handle, and the
    // listing goes on. Worked: the instance area cut to 8 bytes, so `Speed` at P 8 lies outside
    // it and reads as the core's `InvalidBundle`; `Hits` in the retain area still reads 9.
    // Wrong behaviour caught: a listing that fails whole for one variable, a value shown for a
    // variable that does not read, and an error entry with a type or a handle.
    let mut held = Held::new(vec![main_body(1)]);
    held.memory.areas.get_mut()[6].truncate(8);
    held.open_scopes(1);

    let listed = held.listed(1);

    assert_eq!(listed[2], scalar("Hits", "9", "DINT"));
    assert_eq!(
        listed[3],
        json!({
            "name": "Speed",
            "value": "invalid project folder 'the program image's symbol table does not read'",
            "variablesReference": 0,
            "evaluateName": "Speed"
        })
    );
}

#[test]
fn structures_arrays_and_the_io_list_through_the_interpreters_handles() {
    // Specification 34, ARCH-11, and specification 13: a structure's and an array's values take the
    // interpreter's handles and list as the interpreter lists them, and so does the I/O scope.
    // Worked: with one published input the scopes are Locals 1, Globals 2, Retain 3, I/O 4 and
    // Instances 5; the globals give `Levels` 6 and `Origin` 7. `Levels` lists `[0]` 10, `[1]` 20
    // and `[2]` 30; `Origin` lists `X` 3 and `Y` -4; the I/O root lists its three areas with
    // handles 8 to 10, and the inputs the row `start`, TRUE.
    // Wrong behaviour caught: a structure or an array refused or listed in another shape, and an
    // I/O scope that a compiled program lists differently from the interpreter.
    let mut held = Held::new(vec![main_body(1)]);
    held.io = one_input();
    held.open_scopes(1);
    let _ = held.listed(2);

    assert_eq!(
        held.listed(6),
        json!([
            scalar("[0]", "10", "INT"),
            scalar("[1]", "20", "INT"),
            scalar("[2]", "30", "INT")
        ])
    );
    assert_eq!(
        held.listed(7),
        json!([scalar("X", "3", "INT"), scalar("Y", "-4", "INT")])
    );
    assert_eq!(
        held.listed(4),
        json!([
            {"name": "Inputs", "value": "1 items", "variablesReference": 8},
            {"name": "Outputs", "value": "0 items", "variablesReference": 9},
            {"name": "Memory", "value": "0 items", "variablesReference": 10},
        ])
    );
    assert_eq!(
        held.listed(8),
        json!([{
            "name": "start",
            "value": "Bool(true)",
            "variablesReference": 0,
            "evaluateName": "start"
        }])
    );
}

#[test]
fn an_interpreter_handle_names_no_variable_of_a_compiled_program() {
    // Specification 44 §13.2: a compiled program's variables are read through its memory, so a
    // handle of the interpreter's own kinds names none of them and is refused with `the handle
    // names no variable of this program`, as the interpreter refuses a compiled program's handles.
    // Wrong behaviour caught: such a handle answered with an empty list or with the program's
    // globals.
    let held = Held::new(vec![main_body(1)]);
    held.open_scopes(1);

    for handle in [
        VariableHandle::Locals(FrameId(1)),
        VariableHandle::Globals,
        VariableHandle::Retain,
        VariableHandle::Instances,
        VariableHandle::Instance(InstanceId(12)),
    ] {
        let reference = held.handles.borrow_mut().alloc(handle);
        let params = json!({ "variables_reference": reference });
        assert_eq!(
            error(&held.variables(&params)),
            "the handle names no variable of this program"
        );
    }
}

#[test]
fn a_frame_that_is_gone_or_names_no_pou_fails_closed() {
    // Specification 11 §6.9.5: a stale frame fails closed. The locals of a frame that the held
    // stack no longer has fail with `the frame is no longer on the held call stack`; a frame whose
    // POU has no POU symbol, which a debug image never lacks (specification 44 §5.7), fails the
    // stack and its locals with the interpreter's error for metadata that names no POU. Worked:
    // the locals of `Main` (frame 1 of two) after the stack keeps only `SumTo`; a frame of POU 9.
    // Wrong behaviour caught: the locals of another frame listed under a stale handle, and a frame
    // listed without a name.
    let mut held = Held::new(vec![sum_to(2), main_body(1)]);
    held.open_scopes(1);
    held.memory.frames.truncate(1);
    let params = json!({ "variables_reference": 1 });
    assert_eq!(
        error(&held.variables(&params)),
        "the frame is no longer on the held call stack"
    );

    let nameless = StopFrame {
        pou: 9,
        stmt: 2,
        instance: None,
        locals: false,
    };
    let held = Held::new(vec![(nameless, Vec::new())]);
    assert_eq!(
        error(&held.stack()),
        "admitted execution metadata is inconsistent"
    );
    held.open_scopes(1);
    assert_eq!(
        error(&held.variables(&params)),
        "admitted execution metadata is inconsistent"
    );
}
